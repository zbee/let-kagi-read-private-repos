Essentially a fancy proxy to allow Kagi AI access
to my private repositories without giving it PATs.

This worker has secrets with the PATs
(comma-separated), and access is granted to Kagi
via Tokens.

There is a dashboard that is pseudo-protected
where tokens can be made.\
Ideally there is at least one token per Kagi
thread.\
Tokens are deleted on the worker's KV page.

```
let-kagi-read-private-repos
 GET /{owner}/{repo}/[path]?token=...
 GET /{owner}/{repo}/pulls[/:number[.diff]]?token=...
 GET /{owner}/{repo}/issues[/:number]?token=...
 GET /mydash?key=...
```

When any root is accessed (e.g. `/{owner}/{repo}`)
it lists out what is accessible in that root - all
files, all PRs, et cetera.
