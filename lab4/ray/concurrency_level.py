import ray
import requests
import time
import pandas as pd
from typing import Literal

####################################################
# Configuration
####################################################

LLAMA_SERVER = "http://127.0.0.1:8080/completion"

PROMPTS = [
    "Explain what operating systems do.",
    "What is virtual memory?",
    "Explain TCP handshake.",
    "Summarize Linux process scheduling.",
    "What is Docker?",
    "Explain Kubernetes briefly.",
    "Describe Ray architecture.",
    "What is llama.cpp?",
    "Explain quantization in LLM.",
    "What is batch inference?",
    "Describe REST API.",
    "Explain multithreading.",
    "What is GPU acceleration?",
    "Explain cache locality.",
    "Describe MapReduce.",
    "Explain distributed systems.",
    "What is load balancing?",
    "Explain deadlock.",
    "Summarize database indexing.",
    "Explain compiler optimization."
]

####################################################
# Inference Function
####################################################


def llama_inference(prompt: str) -> str | None:
    payload = {
        "prompt": prompt,
        "n_predict": 128,
        "temperature": 0.7
    }

    try:
        response = requests.post(
            LLAMA_SERVER,
            json=payload,
            timeout=300
        )
        response.raise_for_status()
        result = response.json()
        return result.get("content", "")
    except Exception:
        return None


####################################################
# Ray Task
####################################################

@ray.remote
def inference_task(prompt_id: int, prompt: str):
    start = time.time()
    output = llama_inference(prompt)
    end = time.time()
    return {
        "id": prompt_id,
        "prompt": prompt,

        "start_time": start,
        "end_time": end,
        "latency": end - start,

        "output_length": len(output),
        "output": output
    }


####################################################
# Serial Baseline
####################################################

def run_serial(prompts: list[str]):
    print("\nRunning serial inference...\n")
    results = []
    total_start = time.time()

    for idx, p in enumerate(prompts):
        start = time.time()
        output = llama_inference(p)
        end = time.time()
        results.append({
            "id": idx,
            "prompt": p,

            "start_time": start,
            "end_time": end,
            "latency": end-start,

            "output_length": len(output),
            "output": output
        })

    total_end = time.time()

    print(
        f"Serial total time = "
        f"{total_end-total_start:.2f} sec"
    )

    return (results, total_end-total_start)


####################################################
# Ray Parallel
####################################################

def run_parallel(prompts: list[str], concurrency: int = 1):
    print(
        f"\nRunning parallel "
        f"(concurrency={concurrency})\n"
    )
    total_start = time.time()
    results = []
    running = []
    prompt_iter = iter(
        enumerate(prompts)
    )
    # 先填满并发池
    for _ in range(concurrency):
        try:
            idx, p = next(prompt_iter)
            running.append(
                inference_task.remote(
                    idx, p
                )
            )
        except StopIteration:
            break

    # 动态补充任务
    while running:
        finished, running = ray.wait(
            running,
            num_returns=1
        )
        result = ray.get(
            finished[0]
        )
        results.append(result)
        try:
            idx, p = next(prompt_iter)
            running.append(
                inference_task.remote(
                    idx, p
                )
            )
        except StopIteration:
            pass

    total_end = time.time()

    print(
        f"Total time: "
        f"{total_end-total_start:.2f}s"
    )

    return (results, total_end-total_start)


####################################################
# Metrics
####################################################

def summarize(results: list, total_time: float, name: Literal["SERIAL", "PARALLEL"]):
    df = pd.DataFrame(results)
    throughput = len(df) / total_time

    print("\n====================")
    print(name)
    print("====================")
    print(
        f"Requests: {len(df)}"
    )
    print(
        f"Average latency: "
        f"{df['latency'].mean():.2f}s"
    )
    print(
        f"P95 latency: "
        f"{df['latency'].quantile(0.95):.2f}s"
    )
    print(
        f"Throughput: "
        f"{throughput:.2f} req/sec"
    )
    return df


####################################################
# Main
####################################################

if __name__ == "__main__":
    ray.init()
    # serial_results, serial_time = run_serial(PROMPTS)
    parallel_results, parallel_time = run_parallel(PROMPTS)

    # serial_df = summarize(
    #     serial_results,
    #     serial_time,
    #     "SERIAL"
    # )

    parallel_df = summarize(
        parallel_results,
        parallel_time,
        "PARALLEL"
    )

    # serial_df.to_csv(
    #     "serial_results.csv",
    #     index=False
    # )

    parallel_df.to_csv(
        "parallel_results.csv",
        index=False
    )

    print(
        "\nSaved csv files."
    )

    ray.shutdown()
