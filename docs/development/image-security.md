# Container Image Security

## Scope

One server image, `ghcr.io/cagojeiger/grove-storage`, supports Linux amd64 and
arm64. These are platform builds, not separate application roles. Keep the
existing `quick-deploy` chart; this change adds no dedicated Helm chart, execution
role split, GitOps rollout or cluster changes. The current Dockerfile still
packages the Rust server only, not the console's static assets.

## Build And Release Gates

1. CI builds the actual runtime image on native amd64/arm64 runners without
   publishing. A non-root, read-only, capability-dropped `--help` smoke test must
   pass. Existing integration tests remain required; the smoke is not an API test.
2. Trivy scans OS/library vulnerabilities and embedded secrets. HIGH/CRITICAL
   findings block release even without an available fix. Missing/malformed reports
   and missing Rust binary package inventories also block release. No image scan
   ignore list is used. `cargo-auditable` embeds the compiled Rust dependencies;
   a scan of OS packages alone is insufficient.
3. The release follows a successful main **push** CI from this repository and
   checks out that exact source SHA. BuildKit emits SBOM and maximum provenance
   attestations. The actual immutable platform manifests are scanned again.
4. After successful scans, GitHub OIDC signs custom release evidence binding the
   candidate digest to the source SHA, workflow SHA, successful CI run/attempt and
   release run/attempt. `gh attestation verify` verifies the repository, signing
   workflow, predicate type and GitHub-hosted runner. The verified statement must
   match the expected evidence. Source and workflow revisions remain distinct
   because `workflow_run` does not necessarily execute at the source SHA.
5. Both verified platform indexes are assembled into a uniquely named candidate
   index. The final consumer index is also signed and verified before promotion.
   Candidate digests/tags are not production release tags.
6. Recheck current main and the Git tag before promotion. Only a GHCR manifest
   **404** means an unpublished version; authentication/network errors fail.
   Existing version tags are never overwritten by this workflow. After publishing,
   both version and `latest` must resolve to the verified digest. Registry tags are
   not inherently immutable against other writers: restrict package write access.

GitHub attestation verification does not establish a particular SLSA level or
prove absence of application vulnerabilities. SBOM and checksums are not signatures.

## Ongoing Checks

- Daily at 03:17 KST, and on manual dispatch, resolve `latest` once to an immutable
  index, then scan its exact amd64/arm64 manifests using the same policy.
- Retain JSON, readable, SARIF and scan-target reports for 30 days. Publish SARIF
  to GitHub Security on main/release/rescan runs; fork PRs require no write token.
- A failed rescan reports a workflow failure; it does not delete an image or
  automatically change a deployed cluster. Repository notifications must be enabled
  for operators, who assess impact, update dependencies/base images and release.
- Pin external Actions and base images to commit/digest. Dependabot checks Actions,
  Docker, Cargo and npm weekly. PR dependency review rejects new high-severity
  vulnerable dependencies. CI also retains Cargo audit and adds npm audit.
- The existing Cargo audit exception for RUSTSEC-2023-0071 concerns an unused
  sqlx-mysql lockfile dependency; it is not an exception to the image scan policy.

## Deployment Boundary

The runtime uses digest-pinned Distroless `cc-debian13:nonroot`, with numeric
UID/GID `10001:10001` to preserve the existing deployment identity. Its built-in
CA bundle and C/C++ runtime support the Rust binary's dynamic dependencies. There
is no shell or package manager: use HTTP probes, not shell-based exec probes, and
an ephemeral debug container for operational troubleshooting.

Deploy a reviewed version with its recorded digest through existing GitOps; do
not deploy `latest` or candidate tags. Set non-root, read-only root filesystem,
`allowPrivilegeEscalation: false`, `capabilities.drop: [ALL]`, RuntimeDefault
seccomp and a bounded writable `/tmp` emptyDir in `quick-deploy` values. Do not
store database, S3 or account secrets in the image; use existing Secrets.

Older released images may fail the new policy because they lack Rust inventories.
These changes do not attest or repair `0.4.1` retroactively. A new unpublished
version, successful main CI and an actual signed release are required before
claiming the new published artifact passed these gates.

## Verification

Run `python3 -B -m unittest discover -s deploy/tests -v` and `actionlint` locally.
The tests cover findings, incomplete inventories, platform ambiguity, source/CI
evidence mismatches and fail-closed version promotion. Registry publication and
OIDC signing require the trusted GitHub workflow and are not simulated by tests.

Local ARM64 verification on 2026-10-05 used Trivy 0.74.0 with a refreshed database
and no ignore rules. The Debian slim candidate had 57 HIGH/CRITICAL finding
instances across 88 OS packages. The Distroless candidate had zero HIGH/CRITICAL
findings and zero detected secrets across 14 OS packages and 279 compiled Rust
dependencies. This is a point-in-time scan, not a guarantee against future CVEs.
Its local Docker image ID was
`sha256:dac59b5543e143efb979a407a4bfc1474aeb159b906926a99f2d9f43607a433d`;
this is not a published registry manifest digest. With a fresh isolated PostgreSQL
database, account initialization, migrations, HTTP health/readiness and SIGTERM
shutdown passed under non-root, read-only, capability-dropped execution. This
does not establish amd64, S3 end-to-end, OIDC signing or registry release evidence.

References: [Docker attestations](https://docs.docker.com/build/ci/github-actions/attestations/),
[GitHub verification](https://cli.github.com/manual/gh_attestation_verify),
[Distroless runtime](https://github.com/GoogleContainerTools/distroless).
The candidate-scan/evidence workflow follows the responsibilities used by
project-jelly RelayGate and ShiftPV without importing their Helm or runtime roles.
