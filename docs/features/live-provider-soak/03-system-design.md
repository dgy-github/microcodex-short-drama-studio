# Live provider soak design

Status: G6 implementation and local integration passed

`ProviderSoakService` is a Rust desktop adapter over the existing credential,
provider-route, and OpenAI-compatible provider owners. It preflights both
routes, acquires one process-local concurrency guard, and executes the same
minimal structured probe sequentially. Sequential execution keeps the paid
request bound obvious: `iterations × 2`.

Only timing and success/failure classification enter the evidence. The result
contains configured model IDs and a SHA-256 route fingerprint, but no endpoint,
credential, prompt, response body, or provider error string.

The service creates one immutable JSON result through a sibling `.partial`
file and rename. A failed persistence operation returns a stable local-storage
error; it never leaves a successful-looking partial record.
