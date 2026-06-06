import ray
import requests
import time
import pandas as pd
import numpy as np
from typing import List, Dict, Tuple
from enum import Enum
import threading


####################################################
# Configuration
####################################################

LLAMA_SERVER = "http://127.0.0.1:8080/completion"

# Use 30+ prompts for meaningful load balancing comparison
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
    "Explain compiler optimization.",
    "What is a microservice architecture?",
    "Explain CI/CD pipeline.",
    "Describe DNS resolution process.",
    "What is CAP theorem?",
    "Explain blockchain consensus.",
    "Describe RESTful design principles.",
    "What is event-driven architecture?",
    "Explain message queue systems.",
    "Describe content delivery network.",
    "What is service mesh?",
    "Explain container orchestration.",
    "Describe serverless computing.",
    "What is edge computing?",
    "Explain functional programming.",
    "Describe reactive programming."
]

NUM_WORKERS = 4  # Number of Llama worker actors
SCHEDULING_STRATEGIES = ["round_robin", "least_avg_latency"]


####################################################
# Llama Worker Actor
####################################################

@ray.remote(num_cpus=0.5)  # Use fractional CPU per worker
class LlamaWorker:
    """
    Actor that performs inference calls to the Llama server.
    Tracks its own request count and average latency.
    """

    def __init__(self):
        self.request_count = 0
        self.total_latency = 0.0
        self.last_latency = None

    def _do_inference(self, prompt: str) -> str:
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

    def execute_inference(self, prompt: str, start_time: float) -> Dict:
        """
        Execute inference and update internal metrics.
        Returns result dict with timing information.
        """
        output = self._do_inference(prompt)
        end_time = time.time()
        latency = end_time - start_time

        # Update internal metrics
        self.request_count += 1
        self.total_latency += latency
        self.last_latency = latency

        return {
            "worker_id": self._get_id(),
            "prompt": prompt,
            "start_time": start_time,
            "end_time": end_time,
            "latency": latency,
            "output_length": len(output) if output else 0,
            "output": output
        }

    def get_stats(self) -> Dict:
        """Return current worker statistics."""
        avg_latency = self.total_latency / self.request_count if self.request_count > 0 else 0
        return {
            "worker_id": self._get_id(),
            "request_count": self.request_count,
            "total_latency": self.total_latency,
            "avg_latency": avg_latency,
            "last_latency": self.last_latency
        }

    def reset_stats(self) -> None:
        """Reset worker statistics."""
        self.request_count = 0
        self.total_latency = 0.0
        self.last_latency = None

    def _get_id(self) -> int:
        """Get the actor's unique ID."""
        # Ray actors don't have a stable ID by default, use object ref or pass it
        return id(self)


####################################################
# Scheduling Strategies
####################################################

class SchedulingStrategy(Enum):
    ROUND_ROBIN = "round_robin"
    LEAST_AVG_LATENCY = "least_avg_latency"


class RoundRobinScheduler:
    """
    Distributes requests to workers in a round-robin fashion.
    Each request goes to the next worker in sequence.
    """

    def __init__(self, worker_refs: List[ray.ObjectRef]):
        self.worker_refs = worker_refs
        self.current_index = 0
        self.lock = threading.Lock()

    def get_next_worker(self) -> ray.ObjectRef:
        with self.lock:
            worker = self.worker_refs[self.current_index % len(self.worker_refs)]
            self.current_index += 1
            return worker


class LeastAvgLatencyScheduler:
    """
    Assigns each request to the worker with the lowest average latency.
    Requires fetching stats from all workers, so it adds some overhead
    but better distributes load to faster workers.
    """

    def __init__(self, worker_refs: List[ray.ObjectRef]):
        self.worker_refs = worker_refs

    def get_next_worker(self) -> ray.ObjectRef:
        # Fetch stats from all workers concurrently
        stats_refs = [w.get_stats.remote() for w in self.worker_refs]
        stats = ray.get(stats_refs)

        # Find worker with minimum average latency (or most requests if tied/zero)
        best_worker_idx = 0
        best_avg_latency = float('inf')
        best_request_count = 0

        for i, stat in enumerate(stats):
            # Prefer workers with fewer requests if avg latency is similar
            avg_lat = stat['avg_latency']
            req_count = stat['request_count']

            # If no requests yet, pick the one with fewest requests (tie-break)
            if req_count == 0:
                if best_request_count == 0 or req_count < best_request_count:
                    best_worker_idx = i
                    best_avg_latency = avg_lat
                    best_request_count = req_count
            else:
                # Choose worker with lowest avg latency, breaking ties by fewer requests
                if (avg_lat < best_avg_latency or
                    (avg_lat == best_avg_latency and req_count < best_request_count)):
                    best_worker_idx = i
                    best_avg_latency = avg_lat
                    best_request_count = req_count

        return self.worker_refs[best_worker_idx]


####################################################
# Parallel Execution with Load Balancing
####################################################

def run_parallel_with_load_balancing(
    prompts: List[str],
    num_workers: int,
    strategy_name: str
) -> Tuple[List[Dict], Dict]:
    """
    Run inference with load balancing using the specified strategy.

    Returns:
        - List of result dicts
        - Summary dict with worker stats
    """
    strategy = SchedulingStrategy(strategy_name)

    # Create workers
    workers = [LlamaWorker.remote() for _ in range(num_workers)]

    # Initialize scheduler
    if strategy == SchedulingStrategy.ROUND_ROBIN:
        scheduler = RoundRobinScheduler(workers)
    elif strategy == SchedulingStrategy.LEAST_AVG_LATENCY:
        scheduler = LeastAvgLatencyScheduler(workers)
    else:
        raise ValueError(f"Unknown strategy: {strategy_name}")

    total_start = time.time()
    results = []
    pending_futures = []

    # Submit all tasks
    for idx, prompt in enumerate(prompts):
        start = time.time()
        worker = scheduler.get_next_worker()
        future = worker.execute_inference.remote(prompt, start)
        pending_futures.append(future)

    # Collect results as they complete
    for future in pending_futures:
        result = ray.get(future)
        results.append(result)

    total_end = time.time()
    total_time = total_end - total_start

    # Gather final worker statistics
    worker_stats_refs = [w.get_stats.remote() for w in workers]
    worker_stats = ray.get(worker_stats_refs)

    print(f"\n=== {strategy_name.upper()} Scheduling ===")
    print(f"Total time: {total_time:.2f}s")
    print(f"Workers: {num_workers}")
    print(f"Requests: {len(results)}")

    # Print per-worker stats
    print("\n--- Per-Worker Statistics ---")
    for stat in worker_stats:
        print(
            f"  Worker {stat['worker_id']}: "
            f"requests={stat['request_count']}, "
            f"avg_latency={stat['avg_latency']:.3f}s"
        )

    # Cleanup workers
    for w in workers:
        ray.kill(w)

    summary = {
        "strategy": strategy_name,
        "total_time": total_time,
        "num_requests": len(results),
        "worker_stats": worker_stats,
        "results": results
    }

    return results, summary


####################################################
# Metrics Calculation
####################################################

def calculate_metrics(results: List[Dict], strategy_name: str) -> Dict:
    """Calculate throughput, latency metrics from results."""
    if not results:
        return {}

    df = pd.DataFrame(results)
    latencies = df['latency']

    if strategy_name == "least_avg_latency":
        # For parallel execution, throughput is based on max latency (critical path)
        throughput = len(df) / latencies.max()
    else:
        # For serial, throughput is based on total time
        throughput = len(df) / latencies.sum()

    metrics = {
        "strategy": strategy_name,
        "num_requests": len(df),
        "avg_latency": latencies.mean(),
        "p50_latency": latencies.quantile(0.50),
        "p95_latency": latencies.quantile(0.95),
        "p99_latency": latencies.quantile(0.99),
        "max_latency": latencies.max(),
        "min_latency": latencies.min(),
        "throughput": throughput,
        "total_time": latencies.sum() if strategy_name == "round_robin" else latencies.max()
    }

    return metrics


def calculate_worker_distribution(worker_stats: List[Dict]) -> Dict:
    """Calculate distribution metrics across workers."""
    if not worker_stats:
        return {}

    counts = [s['request_count'] for s in worker_stats]
    avg_lats = [s['avg_latency'] for s in worker_stats if s['request_count'] > 0]

    return {
        "num_workers": len(worker_stats),
        "total_requests": sum(counts),
        "min_requests": min(counts),
        "max_requests": max(counts),
        "avg_requests_per_worker": np.mean(counts),
        "std_requests_per_worker": np.std(counts),
        "min_avg_latency": min(avg_lats) if avg_lats else 0,
        "max_avg_latency": max(avg_lats) if avg_lats else 0,
        "avg_avg_latency": np.mean(avg_lats) if avg_lats else 0,
    }


####################################################
# Main
####################################################

def main():
    ray.init(ignore_reinit_error=True)

    num_workers = NUM_WORKERS
    results_data = {}
    all_metrics = []
    all_worker_distributions = []

    for strategy in SCHEDULING_STRATEGIES:
        print(f"\n{'='*60}")
        print(f"Running with {strategy.upper()} strategy")
        print(f"{'='*60}")

        results, summary = run_parallel_with_load_balancing(
            prompts=PROMPTS,
            num_workers=num_workers,
            strategy_name=strategy
        )

        # Calculate metrics
        metrics = calculate_metrics(results, strategy)
        worker_dist = calculate_worker_distribution(summary['worker_stats'])

        results_data[strategy] = {
            'results': results,
            'metrics': metrics,
            'worker_distribution': worker_dist,
            'worker_stats': summary['worker_stats']
        }

        all_metrics.append(metrics)
        all_worker_distributions.append(worker_dist)

        print(f"\n--- Summary for {strategy.upper()} ---")
        print(f"Throughput: {metrics.get('throughput', 0):.3f} req/sec")
        print(f"Avg Latency: {metrics.get('avg_latency', 0):.3f}s")
        print(f"P95 Latency: {metrics.get('p95_latency', 0):.3f}s")
        print(f"Worker Distribution: "
              f"min={worker_dist['min_requests']}, "
              f"max={worker_dist['max_requests']}, "
              f"std={worker_dist['std_requests_per_worker']:.1f}")

    # Create comparison DataFrame
    print(f"\n{'='*60}")
    print("COMPARISON SUMMARY")
    print(f"{'='*60}")

    comparison_df = pd.DataFrame(all_metrics)
    # Clean up for display
    display_cols = ['strategy', 'num_requests', 'avg_latency', 'p95_latency',
                    'throughput', 'total_time']
    comparison_df = comparison_df[[c for c in display_cols if c in comparison_df.columns]]
    print(comparison_df.to_string(index=False))

    # Worker distribution comparison
    print(f"\n{'='*60}")
    print("WORKER DISTRIBUTION COMPARISON")
    print(f"{'='*60}")

    dist_comparison = pd.DataFrame(all_worker_distributions)
    dist_display_cols = ['strategy', 'min_requests', 'max_requests',
                         'avg_requests_per_worker', 'std_requests_per_worker']
    dist_comparison = dist_comparison[[c for c in dist_display_cols if c in dist_comparison.columns]]
    print(dist_comparison.to_string(index=False))

    # Save detailed results to CSV
    for strategy, data in results_data.items():
        df = pd.DataFrame(data['results'])
        csv_filename = f"{strategy}_results.csv"
        df.to_csv(csv_filename, index=False)
        print(f"\nSaved detailed results to {csv_filename}")

        # Save worker stats
        worker_df = pd.DataFrame(data['worker_stats'])
        worker_csv = f"{strategy}_worker_stats.csv"
        worker_df.to_csv(worker_csv, index=False)
        print(f"Saved worker stats to {worker_csv}")

    print("\nAll results saved.")
    ray.shutdown()


if __name__ == "__main__":
    main()
