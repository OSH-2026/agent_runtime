# llama.cpp 部署说明文档

**实验**：Lab4 - LLM 推理部署与优化
**框架**：llama.cpp
**作者**：ubuntu @ VM11727-yjy (Vlab)
**日期**：2026-05-25

---

## 1. 硬件与软件环境

| 项目 | 详情 |
|------|------|
| **CPU** | Intel Xeon Silver 4110 @ 2.10GHz，2 vCPU，支持 AVX-512 |
| **内存** | 6 GB RAM |
| **操作系统** | Ubuntu 22.04 LTS (Linux 7.0.0-3-pve x86_64) |
| **编译器** | gcc 13.3.0 |
| **构建工具** | cmake 3.28.3 |
| **llama.cpp 版本** | b1-c1f1e28 (2026-05-25 主线) |
| **GPU** | 无（纯 CPU 推理） |

---

## 2. 模型信息

| 项目 | 详情 |
|------|------|
| **模型名称** | Qwen2.5-1.5B-Instruct |
| **参数量** | 1.78B |
| **量化格式** | Q4_K_M（主）、Q5_K_M、Q8_0（对比） |
| **文件大小** | Q4_K_M: 1.04 GiB；Q5_K_M: 1.19 GiB；Q8_0: 1.76 GiB |
| **量化方案** | K-quant 混合精度（Q4_K_M/Q5_K_M）、均匀量化（Q8_0） |
| **模型来源** | ModelScope (Qwen/Qwen2.5-1.5B-Instruct-GGUF) |

---

## 3. llama.cpp 编译步骤

### 3.1 获取源码

```bash
git clone --depth=1 https://github.com/ggml-org/llama.cpp.git ~/llama.cpp
cd ~/llama.cpp
```

### 3.2 编译（开启 RPC 支持）

```bash
cmake -B build \
      -DGGML_RPC=ON \
      -DCMAKE_BUILD_TYPE=Release

cmake --build build --config Release -j$(nproc)
```

编译时间约 5-8 分钟（2 vCPU）。

### 3.3 编译产物

| 工具 | 路径 | 用途 |
|------|------|------|
| llama-cli | build/bin/llama-cli | 命令行交互推理 |
| llama-bench | build/bin/llama-bench | 性能基准测试 |
| llama-server | build/bin/llama-server | HTTP REST API 服务器 |
| llama-perplexity | build/bin/llama-perplexity | 困惑度评估 |
| rpc-server | build/bin/rpc-server | RPC 分布式推理后端 |

### 3.4 验证编译

```bash
~/llama.cpp/build/bin/llama-cli --version
# 输出：version: b1-c1f1e28 (2026-05-25)
```

---

## 4. 模型下载

### 4.1 从 ModelScope 下载

HuggingFace 镜像站下载速度约 37 KB/s，改用 ModelScope：

```python
# download_models.py
import urllib.request, os

base_url = ('https://www.modelscope.cn/api/v1/models/'
            'Qwen/Qwen2.5-1.5B-Instruct-GGUF/repo'
            '?Revision=master&FilePath=')
dest_dir = os.path.expanduser('~/models')
os.makedirs(dest_dir, exist_ok=True)

files = [
    'qwen2.5-1.5b-instruct-q4_k_m.gguf',
    'qwen2.5-1.5b-instruct-q5_k_m.gguf',
    'qwen2.5-1.5b-instruct-q8_0.gguf',
]
for fname in files:
    dest = os.path.join(dest_dir, fname)
    print(f'Downloading {fname}...')
    urllib.request.urlretrieve(base_url + fname, dest)
    print(f'  {os.path.getsize(dest) // 1024 // 1024} MiB saved')
```

```bash
python3 download_models.py
```

### 4.2 验证下载

```bash
ls -lh ~/models/
# 预期：
# 1.1G  qwen2.5-1.5b-instruct-q4_k_m.gguf
# 1.2G  qwen2.5-1.5b-instruct-q5_k_m.gguf
# 1.8G  qwen2.5-1.5b-instruct-q8_0.gguf
```

---

## 5. 部署与推理

### 5.1 命令行推理（llama-cli）

```bash
# 部署方式：llama-cli 命令行单次推理
# 运行命令：
~/llama.cpp/build/bin/llama-cli \
  -m ~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf \
  -t 2 -b 256 --ctx-size 512 \
  --temp 0.7 \
  -p "你好，请介绍一下操作系统中的虚拟内存机制。" \
  -n 400
```

**成功推理输出示例**：

```
llm_load_tensors: ggml ctx size =    0.14 MiB
llm_load_tensors: CPU buffer size =  1065.73 MiB
...
llama_perf_context_print: load time =    2341.70 ms
llama_perf_context_print: prompt eval time =     699.09 ms / 23 tokens (30.39 ms/token, 32.90 t/s)
llama_perf_context_print: eval time =   11136.52 ms / 123 runs (90.54 ms/token, 11.04 t/s)

虚拟内存是操作系统提供的一种内存管理技术，它为每个进程创建一个独立的、连续的地址空间...
```

### 5.2 性能基准（llama-bench）

```bash
# 部署方式：llama-bench 基准测试
# 运行命令：
~/llama.cpp/build/bin/llama-bench \
  -m ~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf \
  -p 64 -n 128 -t 2
```

### 5.3 HTTP API 服务器（llama-server）

```bash
# 部署方式：llama-server HTTP 服务
# 运行命令：
~/llama.cpp/build/bin/llama-server \
  -m ~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf \
  -t 2 --port 8080 --host 0.0.0.0

# 测试 API
curl http://localhost:8080/completion \
  -H "Content-Type: application/json" \
  -d '{"prompt": "你好，介绍虚拟内存", "n_predict": 200}'
```

---

## 6. 参数说明

| 参数 | 推荐值 | 说明 |
|------|--------|------|
| `-t / --threads` | `2` | CPU 线程数，设置为物理核心数 |
| `-b / --batch-size` | `256` | prefill 批次大小 |
| `--ctx-size` | `512` | KV cache 上下文长度，影响内存占用 |
| `--temp` | `0.7` | 采样温度，0.7 质量与多样性均衡 |
| `--mmap` | `1`（默认） | 内存映射加载，加快启动速度 |
| `-n` | `400` | 最大生成 token 数 |

---

## 7. 常见问题

**Q：模型下载失败**

```bash
# 检查网络连通性
curl -I https://www.modelscope.cn
```

**Q：内存不足（OOM）**

```bash
# 降低 ctx-size 减少 KV cache 内存
llama-cli -m model.gguf --ctx-size 256 ...
# 或使用更小量化（Q4_K_M 占用最小）
```

**Q：推理速度慢**

```bash
# 确认线程数 = 物理核心数
nproc
llama-cli -m model.gguf -t $(nproc) ...
```
