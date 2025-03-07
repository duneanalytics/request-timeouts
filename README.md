# Reproduction of request timeouts

This is a simplified example of ingestion of blocks from a node provider. It works by:
* Fetching the latest block number.
* Concurrently download the 50k latest blocks, running at most 10k downloads in parallel.
* After around 2 minutes, or 10-20k blocks downloaded, the progress stalls and we get timeouts on all requests.

Note:
* We only see this issue with one node provider.
* Disabling http2, reducing concurrency or removing gzip seems to help (this could also be related to the speed ).
* It is unclear if the issue is on the client or server side.
* This repro works both locally and in our Kubernetes cluster.

# Usage
```commandline
ENDPOINT=...  RUST_LOG=trace cargo run --release 
```
