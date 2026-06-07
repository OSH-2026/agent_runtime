# llama.cpp KV Cache 机制源码解析

陈雨桥 PB24000236

## 1. 问题背景：KV Cache 缓存的到底是什么

Decoder-only Transformer 生成文本时是自回归的：已有上下文先预测下一个 token，新 token 再并入上下文，继续预测下一步。如果每一步都把完整历史序列重新送入模型，历史 token 的各层计算会被反复做一遍，尤其是注意力层里的 Key 和 Value。注意力公式为 $Attention(Q,K,V)=softmax(QK^T/\sqrt d)V$，其中 Q 是当前 token 发起的查询，K 是历史 token 提供的匹配索引，V 是最后被加权汇总的内容；历史 token 一旦算过，它们的 K/V 不会因为后面 token 到来而改变，所以推理时可以把这些 K/V 留在缓存里，下一步只计算新 token 的 Q/K/V，再让新 Q 去看缓存中的历史 K/V。KV Cache 并没有让 decode 阶段的注意力读历史变成 $O(1)$，当前 token 仍要扫过可见历史；它真正省掉的是历史 token 在各层中重复生成 K/V、重复过 MLP、重复构图执行的开销，本质是用显存/内存换计算。

## 2. 源码总览：从 memory 抽象到 KV 张量

当前 llama.cpp 没有把 KV Cache 写成一个孤立数组，而是先抽象成通用的 `llama_memory_i`：KV cache、recurrent memory、hybrid memory 都走这一层接口。`llama_memory_i` 的关键职责是把 batch 切成 ubatch，并准备好本轮计算需要的 memory context，后面的图构建只面对这个 context，不直接操心 cell 怎么分配。源码位置在 `src/llama-memory.h`：

```cpp
// general concept of LLM memory
// the KV cache is a type of LLM memory, but there can be other types
struct llama_memory_i {
    // this callback is used to filter out layers that should not be included in the cache
    using layer_filter_cb = std::function<bool(int32_t il)>;

    // this callback is used to specify which layers should reuse memory from other layers
    // return negative value to indicate that the layer il should not reuse memory
    using layer_reuse_cb = std::function<int32_t(int32_t il)>;

    virtual ~llama_memory_i() = default;

    // split the input batch into a set of ubatches and verify that they can fit into the cache
    // return a context object containing the ubatches and memory state required to process them
    // check the llama_memory_context_i::get_status() for the result
    virtual llama_memory_context_ptr init_batch(
            llama_batch_allocr & balloc,
            uint32_t n_ubatch,
            bool embd_all) = 0;
```

`llama_kv_cache` 是这个接口的一个实现。它内部有两类东西：一类是每层真正存 K/V 的 ggml tensor，另一类是管理这些 tensor 行号的 cell 元数据。层缓存结构定义在 `src/llama-kv-cache.h`，注意 `k_stream` 和 `v_stream` 是对同一个大 tensor 按 stream 切出来的视图：

```cpp
    struct kv_layer {
        // layer index in the model
        // note: can be different from the layer index in the KV cache
        uint32_t il;

        ggml_tensor * k;
        ggml_tensor * v;

        std::vector<ggml_tensor *> k_stream;
        std::vector<ggml_tensor *> v_stream;
    };
```

构造函数里可以看到 K/V 张量按三维申请，形状就是每个 token 的 GQA 后维度、cell 数、stream 数。随后每个 stream 再用 `ggml_view_2d` 切出独立视图，这意味着stream是张量第三维上的实际分区：

```cpp
        ggml_tensor * k = has_k ? ggml_new_tensor_3d(ctx, type_k, n_embd_k_gqa, kv_size, n_stream) : nullptr;
        ggml_tensor * v = has_v ? ggml_new_tensor_3d(ctx, type_v, n_embd_v_gqa, kv_size, n_stream) : nullptr;

        has_k && ggml_format_name(k, "cache_k_l%d", il);
        has_v && ggml_format_name(v, "cache_v_l%d", il);

        std::vector<ggml_tensor *> k_stream;
        std::vector<ggml_tensor *> v_stream;

        for (uint32_t s = 0; s < n_stream; ++s) {
            k_stream.push_back(has_k ? ggml_view_2d(ctx, k, n_embd_k_gqa, kv_size, k->nb[1], s*k->nb[2]) : nullptr);
            v_stream.push_back(has_v ? ggml_view_2d(ctx, v, n_embd_v_gqa, kv_size, v->nb[1], s*v->nb[2]) : nullptr);
        }
```

## 3. Cell：KV Cache 的账本

真正的 K/V 数据不在 cell 里，cell 更像一本账：第 `i` 个缓存位置是否被占用、对应哪个逻辑位置 `pos`、属于哪些 sequence、有没有等待处理的位置偏移 `shift`。`src/llama-kv-cells.h` 的注释直接点明了它支持一个 cell 同时属于多个 sequence，这就是 prefix sharing 或 beam search 分叉时能复用缓存的基础：

```cpp
// meta information about KV cells that can be part of multiple sequences at the same time
// TODO: add unit tests
class llama_kv_cells {
public:
    void reset() {
        for (uint32_t i = 0; i < pos.size(); ++i) {
            pos[i]   = -1;
            ext[i].reset();
            shift[i] =  0;
            seq[i].reset();
        }

        has_shift = false;

        used.clear();

        for (uint32_t s = 0; s < LLAMA_MAX_SEQ; ++s) {
            seq_pos[s].clear();
        }
    }
```

cell 的关键字段集中在类尾部。`pos[i] == -1` 表示空位，`used` 让实现能快速知道当前占用范围，`seq[i]` 是 `bitset`，记录这个 cell 被哪些 sequence 使用，`seq_pos` 则是位置到出现次数的映射，因为视觉模型或缓存复用过程中同一 sequence 的同一位置可能出现多次：

```cpp
private:
    bool has_shift = false;

    // set of indices of used cells (i.e. pos[i] != -1, allowed to not have any seq_id)
    std::set<uint32_t> used;

    std::vector<llama_pos> pos;

    // stores extra info per cell
    std::vector<llama_kv_cell_ext> ext;

    // this array accumulates any applied shifts to the pos array since the last reset_shift() call
    // this is used to queue multiple updates to the pos array, which in the end can be applied in one go:
    //
    //   cells.pos_add(x, shift_x);
    //   cells.pos_div(y, shift_y);
    //   ...
    //
    //   if (cells.has_shift()) {
    //      for (int i = 0; i < n; ++i) {
    //          auto shift_i = cells.get_shift(i);
    //          ...
    //      }
    //      cells.reset_shift();
    //   }
    //
    std::vector<llama_pos> shift;

    using seq_set_t = std::bitset<LLAMA_MAX_SEQ>;

    // the bitset seq[i] tells us which sequences are currently occupying the i-th cell
    std::vector<seq_set_t> seq;

    // the set seq_pos[s][p] tells us how many times the position p is currently present for sequence s
    // if the position p is not present, seq_pos[s][p] is not set
    // this way seq_pos[s].begin() and seq_pos[s].rbegin() give us the min/max positions currently in the cache
    //
    // note that we cannot a use an std::set because in some cases a position can occur more than once for the same seq:
    //  - during performing a cache reuse via (rm + add)
    //  - some vision models have input embeddings with repeating positions
    //
    std::map<llama_pos, int> seq_pos[LLAMA_MAX_SEQ];
```

从工程角度看，cell和 K/V tensor 分离很关键：CPU 侧可以先改 cell 元数据，确定 token 写到哪个 cell；后端张量计算再用这些下标把 K/V 写入对应行。这样做的代价是源码要维护 `pos`、`seq`、`head`、`shift` 等状态一致性，好处是跨 CPU、CUDA、Metal、Vulkan 等后端时，缓存管理逻辑不用散落进各个 kernel。

## 4. Batch 到 Slot：先分配位置，再执行计算

用户调用 `llama_decode()` 后，真正进入上下文对象的是 `ctx->decode(batch)`。在 `llama_context::decode` 里，计算前会先处理 pending memory update，然后调用 `memory->init_batch` 为本轮 batch 找位置；如果找不到位置，它会尝试做一次 cache optimization，再失败才返回 1：

```cpp
    // handle any pending shifts/copies
    memory_update(false);

    llama_memory_context_ptr mctx;

    while (true) {
        mctx = memory->init_batch(*balloc, cparams.n_ubatch, output_all);
        if (!mctx) {
            return -2;
        }
```

KV cache 自己的 `init_batch` 做两件事：先由 `llama_batch_allocr` 切 ubatch；再调用 `prepare` 预分配 slot。单 stream 用 `split_simple`，多 stream 用 `split_equal`，因为多 stream 时需要保证不同 sequence 的 token 能对齐到各自 stream：

```cpp
llama_memory_context_ptr llama_kv_cache::init_batch(
            llama_batch_allocr & balloc,
            uint32_t n_ubatch,
            bool embd_all) {
    GGML_UNUSED(embd_all);

    do {
        balloc.split_reset();

        std::vector<llama_ubatch> ubatches;
        while (true) {
            auto ubatch = n_stream == 1 ? balloc.split_simple(n_ubatch) : balloc.split_equal(n_ubatch, true);

            if (ubatch.n_tokens == 0) {
                break;
            }

            ubatches.push_back(std::move(ubatch)); // NOLINT
        }
```

`prepare` 的写法很谨慎：它会对每个 ubatch 调 `find_slot`，并临时 `apply_ubatch` 模拟占用；但函数返回前又把 cell 和 head 恢复到原状态。也就是说，`prepare` 只是验证并记录将来该放哪，真正写入元数据发生在 memory context 的 `apply()` 阶段。这种设计避免了多个 ubatch 互相占用位置，却又不让预处理提前污染真实 cache 状态：

```cpp
    for (const auto & ubatch : ubatches) {
        // only find a suitable slot for the ubatch. don't modify the cells yet
        const auto sinfo_new = find_slot(ubatch, false);
        if (sinfo_new.empty()) {
            success = false;
            break;
        }

        // remember the position that we found
        res.push_back(sinfo_new);

        // store the old state of the cells in the recovery stack
        {
            state_t state = { sinfo_new, v_heads, {} };

            for (uint32_t s = 0; s < sinfo_new.n_stream(); ++s) {
                auto & cells = v_cells[sinfo_new.strm[s]];

                state.v_cells.push_back(cells.cp(sinfo_new.idxs[s]));
            }

            states.push_back(std::move(state));
        }

        // now emplace the ubatch
        apply_ubatch(sinfo_new, ubatch);
    }
```

`find_slot` 是 KV Cache 分配的核心。每个 stream 有自己的 `head`，从上次位置继续向后扫描；遇到末尾则回到 0。空 cell 可以用；如果 cell 只属于一个 sequence，且在 SWA 规则下已经不可见，也可以复用。这里的环形缓冲区不通过 `head` 在固定大小的 cell 数组里寻找可覆盖位置：

```cpp
                bool can_use = cells.is_empty(idx);

                if (!can_use && cells.seq_count(idx) == 1) {
                    const llama_pos pos_cell = cells.pos_get(idx);

                    // (disabled) causal mask
                    // note: it's better to purge any "future" tokens beforehand
                    //if (cells.seq_has(idx, seq_id)) {
                    //    can_use = pos_cell >= pos;
                    //}

                    if (!can_use) {
                        const llama_seq_id seq_id_cell = cells.seq_get(idx);

                        // SWA mask
                        if (llama_hparams::is_masked_swa(n_swa, swa_type, pos_cell, cells.seq_pos_max(seq_id_cell) + 1)) {
                            can_use = true;
                        }
                    }
                }
```

找到 slot 后，`apply_ubatch` 把 ubatch 的 `pos`、2D 扩展位置和 `seq_id` 写进 cell。如果覆盖了 SWA 中可复用的旧 cell，还会记录被覆盖 sequence 的最大位置，随后清掉更早的不连续残留，保持每个 sequence 在 cache 中的 `[pos_min, pos_max]` 区间连续：

```cpp
            if (!cells.is_empty(idx)) {
                assert(cells.seq_count(idx) == 1);

                const llama_seq_id seq_id = cells.seq_get(idx);
                const llama_pos    pos    = cells.pos_get(idx);

                seq_pos_max_rm[seq_id] = std::max(seq_pos_max_rm[seq_id], pos);

                cells.rm(idx);
            }

            cells.pos_set(idx, ubatch.pos[i]);

            if (ubatch.is_pos_2d()) {
                llama_kv_cell_ext ext {
                    /*.x =*/ ubatch.pos[i + ubatch.n_tokens*2],
                    /*.y =*/ ubatch.pos[i + ubatch.n_tokens],
                };
                cells.ext_set(idx, ext);
            }

            for (int32_t s = 0; s < ubatch.n_seq_id[i]; s++) {
                cells.seq_add(idx, ubatch.seq_id[i][s]);
            }
```

## 5. 计算图读写：`ggml_set_rows` 与 view

cell 决定写到哪一行，但真正的 K/V 写入在计算图里完成。`src/llama-graph.cpp` 中，attention 构图先把当前层的 `q_cur`、`k_cur`、`v_cur` 加进图，然后调用 memory context 的 `cpy_k/cpy_v` 写入 cache，再通过 `get_k/get_v` 取出当前可见的历史缓存送入 attention：

```cpp
    // store to KV cache
    {
        const auto & k_idxs = inp->get_k_idxs();
        const auto & v_idxs = inp->get_v_idxs();

        ggml_build_forward_expand(gf, mctx_cur->cpy_k(ctx0, k_cur, k_idxs, il));
        ggml_build_forward_expand(gf, mctx_cur->cpy_v(ctx0, v_cur, v_idxs, il));
    }

    const auto & kq_mask = inp->get_kq_mask();

    ggml_tensor * q = q_cur;
    ggml_tensor * k = mctx_cur->get_k(ctx0, il);
    ggml_tensor * v = mctx_cur->get_v(ctx0, il);

    ggml_tensor * cur = build_attn_mha(q, k, v, kq_b, kq_mask, sinks, v_mla, kq_scale, il);
```

`cpy_k` 的关键是把当前 token 的 K reshape 成二维行矩阵，再用 `ggml_set_rows` 按 `k_idxs` 写入 cache；多 stream 时还会先把三维 K cache reshape 成二维，因为下标是全局行号：

```cpp
ggml_tensor * llama_kv_cache::cpy_k(ggml_context * ctx, ggml_tensor * k_cur, ggml_tensor * k_idxs, int32_t il, const slot_info & sinfo) const {
    GGML_UNUSED(sinfo);

    const int32_t ikv = map_layer_ids.at(il);

    ggml_tensor * k = layers[ikv].k;

    const int64_t n_embd_head = k_cur->ne[0];
    const int64_t n_head      = k_cur->ne[1];
    const int64_t n_tokens    = k_cur->ne[2];

    const int64_t n_embd_gqa = n_embd_head*n_head;

    // we can merge dims 0 and 1
    // TODO: add ggml helper function for this?
    GGML_ASSERT(ggml_row_size(k_cur->type, n_embd_head) == k_cur->nb[1]);

    k_cur = ggml_view_2d(ctx, k_cur, n_embd_gqa, n_tokens, k_cur->nb[2], 0);

    const int64_t n_stream = k->ne[2];

    if (n_stream > 1) {
        const int64_t kv_size = get_size();

        assert(n_embd_gqa == k->ne[0]);
        assert(kv_size    == k->ne[1]);

        // merge the buffer across all streams because the idxs are global
        k = ggml_reshape_2d(ctx, k, n_embd_gqa, kv_size*n_stream);
    }

    // store the current K values into the cache
    return ggml_set_rows(ctx, k, k_cur, k_idxs);
}
```

`get_k` 则不复制数据，只创建 `ggml_view_4d`。它把 cache 解释成 `[head_dim, kv_heads, n_kv, n_streams_in_slot]` 的视图，供 attention 读取；这里的 `n_kv` 会做 padding，让图形状尽量稳定，便于后端复用计算图：

```cpp
ggml_tensor * llama_kv_cache::get_k(ggml_context * ctx, int32_t il, uint32_t n_kv, const slot_info & sinfo) const {
    const int32_t ikv = map_layer_ids.at(il);

    auto * k = layers[ikv].k;

    const uint64_t kv_size      = get_size();
    const uint64_t n_embd_k_gqa = k->ne[0];

    assert(n_embd_k_gqa == hparams.n_embd_k_gqa(il));

    const uint32_t ns = sinfo.s1 - sinfo.s0 + 1;

    return ggml_view_4d(ctx, k,
            hparams.n_embd_head_k(il), hparams.n_head_kv(il), n_kv, ns,
            ggml_row_size(k->type, hparams.n_embd_head_k(il)),
            ggml_row_size(k->type, n_embd_k_gqa),
            ggml_row_size(k->type, n_embd_k_gqa*kv_size),
            ggml_row_size(k->type, n_embd_k_gqa*kv_size)*sinfo.s0);
}
```

V cache 稍微复杂，因为 llama.cpp 支持转置布局。不开 Flash Attention 时常见的是 `v_trans = true`，读取 V 时视图维度会把 `n_kv` 放到第 0 维，这样可以配合后端的矩阵布局；开 FA 时 V 不转置，逻辑更接近 K。这也是状态保存/恢复代码里 V 比 K 更绕的原因。

## 6. Stream、SWA 与 K-shift：缓存不是只进不出

stream 解决的是多 sequence 并行时的隔离问题。构造函数里 `n_stream` 由 `unified` 决定：统一 KV 时所有 sequence 映射到 stream 0；非统一时每个 sequence 可以映射到自己的 stream。源码直接初始化了这张 `seq_to_stream` 表：

```cpp
    // by default, all sequence ids are mapped to the 0th stream
    seq_to_stream.resize(LLAMA_MAX_SEQ, 0);

    if (n_stream > 1) {
        seq_to_stream.resize(n_stream, 0);
        for (uint32_t s = 0; s < n_stream; ++s) {
            seq_to_stream[s] = s;
        }
    }
```

同一个 stream 内复制 sequence 不需要搬 K/V 数据，只要给 cell 增加一个 `seq_id`；跨 stream 复制则必须复制真实 tensor，但 llama.cpp 不在 `seq_cp` 里马上做，而是把源/目标 stream 记入 `sc_info`，等下一次 `update` 统一搬运：

```cpp
    // enqueue the copy operation - the buffer copy will be performed during the next update
    sc_info.ssrc.push_back(s0);
    sc_info.sdst.push_back(s1);
```

`update` 先处理这种 stream copy，再处理 K-shift。跨 stream copy 会同步上下文并逐层调用 `ggml_backend_tensor_copy`：

```cpp
            for (uint32_t il = 0; il < layers.size(); ++il) {
                const auto & layer = layers[il];

                ggml_backend_tensor_copy(layer.k_stream[ssrc], layer.k_stream[sdst]);

                if (layer.v_stream[ssrc]) {
                    ggml_backend_tensor_copy(layer.v_stream[ssrc], layer.v_stream[sdst]);
                }
            }
```

SWA，即 Sliding Window Attention滑动窗口注意力，简单来说就是保证只能看到固定窗口长度的上下文，是一种常见的稀疏注意力实现方式，会同时影响能不能复用 cell和attention 能不能看见某位置。前者体现在 `find_slot`：窗口外且只属于一个 sequence 的旧 cell 可以覆盖；后者体现在 mask 构造，源码先把 mask 填成 `-INFINITY`，只有同 sequence、非 future、未被 SWA 屏蔽的位置才改成有效值：

```cpp
                // mask different sequences
                if (s0 != s1) {
                    continue;
                }

                // mask future tokens
                if (cparams.causal_attn && p0 > p1) {
                    continue;
                }

                // apply SWA if any
                if (llama_hparams::is_masked_swa(n_swa, swa_type, p0, p1)) {
                    continue;
                }

                data[idst + i0] = hparams.use_alibi ? -std::abs(p0 - p1) : 0.0f;
```

K-shift 处理的是 RoPE 位置变了以后，缓存里的 K 如何跟着改。`seq_add` 和 `seq_div` 会改 cell 的 `pos`，同时把偏移累积到 `shift`；`update` 发现 `get_has_shift()` 后构建一张专门的 shift 图，只对 K 的 RoPE 部分做旋转修正，最后清空 shift。核心入口如下：

```cpp
    if (do_shift) {
        if (!get_can_shift()) {
            GGML_ABORT("The current KV cache / model configuration does not support K-shift");
        }

        LLAMA_LOG_DEBUG("%s: applying K-shift\n", __func__);

        // apply K-shift if needed
        if (hparams.rope_type != LLAMA_ROPE_TYPE_NONE) {
            ggml_backend_sched_reset(sched);

            auto * res = lctx->get_gf_res_reserve();

            res->reset();

            auto * gf = build_graph_shift(res, lctx);
```

`build_graph_shift` 只取 K cache 中需要旋转的位置维度，用 `build_rope_shift` 生成修正图；这再次说明 K-shift 不是搬移 K/V 数据，而是在原缓存上修正 K 向量里编码过的位置信息：

```cpp
        ggml_tensor * k =
            ggml_view_3d(ctx, layer.k,
                n_rot, n_head_kv, get_size()*n_stream,
                ggml_row_size(layer.k->type, n_embd_head_k),
                ggml_row_size(layer.k->type, n_embd_k_gqa),
                ggml_row_size(layer.k->type, n_embd_nope));

        ggml_tensor * cur = build_rope_shift(cparams, ctx, k, inp->k_shift, inp->k_rot, rope_factors, freq_base_l, freq_scale_l, il);

        ggml_build_forward_expand(gf, cur);
```

## 7. 序列操作与状态保存：让缓存能复用、能恢复

KV Cache 的公共操作集中在 `seq_rm`、`seq_cp`、`seq_keep`、`seq_add`、`seq_div`：删除某段缓存、复制 sequence、只保留某个 sequence、平移位置、压缩位置。这些 API 是多轮对话、beam search、上下文复用和 prompt cache 的基础。例如 `seq_rm` 删除 cell 后会把 `head` 指到释放出的更早位置，下一次 `find_slot` 就能更快找到空位：

```cpp
        // If we freed up a slot, set head to it so searching can start there.
        if (new_head != cells.size() && new_head < head) {
            head = new_head;
        }
```

状态保存分两块：先写 cell 元数据，再写各层 K/V tensor。`state_write` 会按 stream 扫描非空 cell，并把连续 cell 合成 range，减少零散 I/O：

```cpp
        for (uint32_t i = 0; i < cells.size(); ++i) {
            if (!cells.is_empty(i) && (seq_id == -1 || cells.seq_has(i, seq_id))) {
                ++cell_count;
                if (cell_range_begin == cells.size()) {
                    cell_range_begin = i;
                }
            } else {
                if (cell_range_begin != cells.size()) {
                    cr.data.emplace_back(cell_range_begin, i);
                    cell_range_begin = cells.size();
                }
            }
        }
```

写数据时 K 比较直接：每个 cell 一行，按 range 写出。V 如果是转置布局，则必须按 embedding 维度逐行取 cell 区间，不能简单整段 memcpy；这段源码解释了为什么 KV 状态读写代码看起来比保存一个数组复杂得多：

```cpp
            // For each row, we get the element values of each cell
            for (uint32_t j = 0; j < n_embd_v_gqa; ++j) {
                // Read each range of cells of v_size_el length and write out
                for (const auto & range : cr.data) {
                    const size_t range_size = range.second - range.first;
                    const size_t src_offset = (range.first + j * kv_size) * v_size_el;
                    const size_t buf_size = range_size * v_size_el;
                    io.write_tensor(v, src_offset, buf_size);
                }
            }
```

## 8. 与 vLLM PagedAttention 的对照

llama.cpp 的 KV Cache 更像连续 cell 数组 + 环形扫描 + 多后端通用 view/set_rows：管理单位是一个 token 对应的 cell，内存布局连续，源码复杂度主要花在 cell 元数据、stream 隔离、SWA/K-shift、状态保存这些工程细节上。vLLM 的 PagedAttention 则把 KV Cache 分成固定大小 block/page，用 block table 把逻辑 token 位置映射到物理块；它更适合高并发在线服务，长短请求混跑时内存利用率更高，prefix sharing 也能以 block 为单位做得更灵活，但代价是 block table、分页调度和专门 GPU kernel 的复杂度更高。简言之，llama.cpp 选择的是本地推理和多后端移植友好的连续缓存，vLLM 选择的是服务端吞吐优先的分页缓存；二者都在管理 K/V，只是一个像紧凑的数组账本，一个像操作系统的分页表。

