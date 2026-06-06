import ray
import requests
import time
import pandas as pd
from typing import List

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


def llama_inference(prompt: str) -> str:

    payload = {

        "prompt": prompt,
        "n_predict": 128,
        "temperature": 0.7

    }

    response = requests.post(
        LLAMA_SERVER,
        json=payload,
        timeout=300
    )

    response.raise_for_status()

    result = response.json()

    return result.get("content", "")


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

def run_serial(prompts: List[str]):

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

    return results


####################################################
# Ray Parallel
####################################################

def run_parallel(prompts: List[str]):

    print("\nRunning Ray parallel inference...\n")

    total_start = time.time()

    futures = [

        inference_task.remote(i, p)

        for i, p in enumerate(prompts)

    ]

    results = ray.get(futures)

    total_end = time.time()

    print(
        f"Parallel total time = "
        f"{total_end-total_start:.2f} sec"
    )

    return results


####################################################
# Metrics
####################################################

def summarize(results, name):

    df = pd.DataFrame(results)

    if name == "PARALLEL":
        throughput = len(df)/df["latency"].max()
    else:
        throughput = len(df)/df["latency"].sum()

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

    serial_results = run_serial(PROMPTS)

    parallel_results = run_parallel(PROMPTS)

    serial_df = summarize(
        serial_results,
        "SERIAL"
    )

    parallel_df = summarize(
        parallel_results,
        "PARALLEL"
    )

    serial_df.to_csv(
        "serial_results.csv",
        index=False
    )

    parallel_df.to_csv(
        "parallel_results.csv",
        index=False
    )

    print(
        "\nSaved csv files."
    )

    ray.shutdown()
