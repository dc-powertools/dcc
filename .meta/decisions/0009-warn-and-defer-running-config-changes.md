# 0009: Warn And Defer Detectable Running-Container Configuration Changes

Status: Accepted policy and snapshot/fingerprint approach; detailed design pending

Date: 2026-09-28

Authority: user amendment during T-0095's configuration-drift discussion.

## Decision

When configuration changes while a container is running, warn and defer changes.
Apply this to any easily detectable configuration changes, not only `forwardPorts`.
Do not require exhaustive detection of changes that are difficult to observe.
Do not automatically restart or recreate the container to apply new configuration.

Continue allowing access using the running container's configuration. Apply changes
on explicit recreation, with an image rebuild when build-time changes require it.
Detection must not itself apply pending configuration or mutate shared runtime assets.

## Accepted Snapshot And Fingerprint Approach

The user suggested injecting a configuration hash during image build and having the
client submit the current hash to the supervisor for comparison. This is a suggested
mechanism, not a requirement for a particular hash format or control verb.

Build and launch are distinct boundaries in the current implementation. Runtime
configuration is loaded again in `RuntimePlan::prepare`; substitutions and runtime
settings may legitimately differ from image-build inputs. A build fingerprint can
identify image staleness, but cannot alone describe which configuration was applied
to a particular running container.

The user accepted the following refinement after the build/launch distinction and
need to retain original configuration were explained:

- Capture the effective configuration at container creation, with a fingerprint
  that can be compared against current readily available configuration inputs.
- Retain enough of that snapshot to reuse the original configuration for operations
  that otherwise reread mutable files, including attach hooks and named commands.
- Compare on reuse and warn on a mismatch; do not update the baseline until recreation.
- Treat image-build fingerprinting separately if added. A warning need not enumerate
  every changed field; a hash comparison can support a generic drift warning.
- Use already loaded configuration/inheritance and inexpensive inputs; do not add
  remote polling or exhaustive workspace hashing merely to detect all possible drift.

A launch-time snapshot and fingerprint are therefore the accepted reuse baseline;
separate image-build fingerprinting remains optional. Exact representation and
comparison protocol remain design details.

A hash detects a difference but cannot recover the previous configuration by itself.
`RuntimePlan::prepare` currently creates cache/state paths and clears/rewrites startup
hook assets before running-container lookup. Its ordering needs review to honor
deferral; appending a comparison after that preparation is insufficient.

Exact fingerprint boundaries, snapshot storage, treatment of unavailable/invalid
current configuration, and compatibility with older containers remain design work.
New host/supervisor protocol must follow decision 0004's compatibility boundary.
No product-code changes or runtime reproduction are authorized by this record.
