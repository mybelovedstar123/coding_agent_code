# Security policy

Please report vulnerabilities privately through the **Report a vulnerability** button on this repository's
[Security tab](https://github.com/mybelovedstar123/coding_agent_code/security), not in a public issue.

The areas that matter most are:

- the `run_python` sandbox in `src/pytool.rs`, which runs programs written by a language model;
- the tool executor, which must only run tools offered for the current request;
- the separation of one user's records from another's;
- any path by which an API key could reach a log, a response or a tool endpoint.

Include the steps to reproduce the problem and what an attacker would gain. Only the latest commit on `main` is
supported.
