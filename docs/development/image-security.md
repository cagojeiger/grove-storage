# Container Image Security

## Scope

One server image, `ghcr.io/cagojeiger/grove-storage`, supports Linux amd64 and
arm64. These are platform builds, not separate application roles. Keep the
existing `quick-deploy` chart; this change adds no dedicated Helm chart, execution
role split, GitOps rollout or cluster changes. The Dockerfile packages the Rust
server, production console assets and the matching npm lockfile in one image.
Node and build tools remain in build stages, not in the runtime.

## Build And Release Gates

1. CI builds the actual runtime image on native amd64/arm64 runners without
   publishing. A non-root, read-only, capability-dropped `--help` smoke runs first.
   `scripts/e2e-image.py` then uses the actual image and a fresh PostgreSQL fixture
   for hidden-TTY account initialization, readiness, HTTPS console login/logout,
   navigation, Swagger, host isolation and SIGTERM. The same console-enabled image
   also passes standard S3 SDK presigned PUT/GET, Range, signature/expiry rejection,
   physical MinIO byte verification, existing credentials and presigned GET after
   restart, and delete after browser logout. Installed migration versions and
   SHA384 checksums match the candidate sources. The fixture
   needs boto3 and builds the existing pinned test-only MinIO image.
   Desktop/mobile screenshots and `s3.json` bind runtime evidence to the inspected
   local image ID. Broader S3/NoteGate integration tests remain separate gates.
2. Trivy reports all severity levels for OS/library vulnerabilities and embedded
   secrets. HIGH/CRITICAL
   findings block release even without an available fix. Missing/malformed reports
   and missing Rust or frontend package inventories also block release. No image scan
   ignore list is used. `cargo-auditable` embeds the compiled Rust dependencies;
   a scan of OS packages alone is insufficient. `/app/inventory/frontend/package-lock.json`
   retains npm dependency information because minified browser bundles alone do
   not provide a reliable package inventory. Trivy's image scanner does not read
   npm lockfiles: the gate extracts this file from the exact image without running
   it, scans it with `trivy fs`, and joins the results for enforcement and SARIF.
   Both original reports are preserved. Node is not installed to enable scanning.
3. The release follows a successful main **push** CI from this repository and
   checks out that exact source SHA. BuildKit emits SBOM and maximum provenance
   attestations. The actual immutable platform manifests are scanned again and
   pass the packaged-console/API fixture before any signing or promotion.
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

`GROVE_CONSOLE_ORIGIN` enables the packaged console on its dedicated HTTPS
host. TLS termination preserves `Host`; forwarded host headers are not trusted.
The management host does not serve user objects, and other hosts do not serve
the console or its browser-only identity/command APIs. Packaging does not create
an ingress or a default administrator password. Runtime configuration is described
in [container connections](../stack/README.md#컨테이너-연결).

Deploy a reviewed version with its recorded digest through existing GitOps; do
not deploy `latest` or candidate tags. Set non-root, read-only root filesystem,
`allowPrivilegeEscalation: false`, `capabilities.drop: [ALL]`, RuntimeDefault
seccomp and a bounded writable `/tmp` emptyDir in `quick-deploy` values. Do not
store database, S3 or account secrets in the image; use existing Secrets.

Older released images may fail the new policy because they lack Rust or frontend inventories.
These changes do not attest or repair `0.4.1` retroactively. A new unpublished
version, successful main CI and an actual signed release are required before
claiming the new published artifact passed these gates.

## Verification

Run `python3 -B -m unittest discover -s deploy/tests -v` and `actionlint` locally.
The tests cover findings, incomplete inventories, platform ambiguity, source/CI
evidence mismatches and fail-closed version promotion. Registry publication and
OIDC signing require the trusted GitHub workflow and are not simulated by tests.

Local ARM64 verification on 2026-10-06 used Trivy 0.74.0 with a refreshed database
and no ignore rules. The packaged image had zero HIGH/CRITICAL findings and zero
detected secrets across 14 OS packages, 282 compiled Rust dependencies and 301
production npm dependencies. This is a point-in-time scan, not a guarantee against
future CVEs. Its local Docker image ID was
`sha256:00b487184769efb7dde209052ce7c268f67166ae829e3fc4f4dfc7a5aa76f344`;
this is not a published registry manifest digest. With a fresh isolated PostgreSQL
database, account initialization, readiness, HTTPS console login/logout, resource
navigation, Swagger, desktop/mobile rendering, host isolation and SIGTERM passed
under non-root, read-only, capability-dropped execution. This does not establish
amd64, S3 end-to-end, OIDC signing or registry release evidence.

### Candidate Checkpoint (2026-10-08)

The current working-tree ARM64 image is
`sha256:a15bc4b2c5e684e994bef6d9bc468dd6a6eb9245938e1a1025b8baa9880a1a81`.
This is a local Docker image ID, not a registry manifest digest or published release.
The hardened runtime passed fresh account initialization, all eight migration
checksums, HTTPS console login/logout, secure cookies, CSP, Swagger, desktop/mobile
rendering, host isolation and graceful shutdown. Standard S3 credentials, presigned
PUT/GET, Range, signature/expiry rejection and physical MinIO bytes passed without
a browser session. Restart preserved the existing credentials, object and already
issued GET URL.

Trivy 0.74.0 downloaded its database on 2026-10-08 and inspected 14 OS packages,
282 compiled Rust dependencies and 301 production npm dependencies from that image.
The combined report passed the repository policy with zero HIGH/CRITICAL findings
and zero detected secrets. It retained 24 MEDIUM and eight LOW findings without
suppression. Policy success does not mean a vulnerability-free image.

The local evidence does not establish native amd64 execution, same-commit GitHub
CI, registry publication, signing, CLI release assets or production deployment.

References: [Docker attestations](https://docs.docker.com/build/ci/github-actions/attestations/),
[GitHub verification](https://cli.github.com/manual/gh_attestation_verify),
[Distroless runtime](https://github.com/GoogleContainerTools/distroless),
[Trivy npm coverage](https://trivy.dev/docs/v0.74/guide/coverage/language/nodejs/).
The candidate-scan/evidence workflow follows the responsibilities used by
RelayGate and NoteGate without importing their Helm or runtime roles.

## Known Dependency Advisory

On 2026-10-07, Dependabot reported the medium-severity
[GHSA-hp3w-g68c-fv3c](https://github.com/advisories/GHSA-hp3w-g68c-fv3c)
for `sprintf-js` through Swagger UI's `remarkable` / `argparse` dependency chain.
The advisory had no patched version. No suppression is configured; lower-severity
findings remain in scan evidence and do not pass as a vulnerability-free result.
