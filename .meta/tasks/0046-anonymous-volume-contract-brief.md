# T-0046 Brief: Anonymous Volume Mount Contract

## Identity And Source

- Task ID: T-0046
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

Feature-declared anonymous volumes are emitted as valid anonymous Docker mounts, while
named volumes retain their source.

## Background

`FeatureMount::to_mount_string` always emits `source=...`. The test named
`parse_label_volume_mount_omits_source` claims an empty source is omitted but asserts
`source=` is present, locking in the opposite behavior from the Docker volume contract.

## Scope

In scope:

- Omit the source field for anonymous `type=volume` mounts.
- Preserve explicit sources for named volumes and bind mounts.
- Test the serialized Docker argument as a public integration boundary, including
  escaping or validation cases already supported by the parser.

Out of scope:

- Redesigning Feature label metadata or introducing new mount types.

## Acceptance Criteria

- [ ] An anonymous volume produces `type=volume,target=<path>` with no empty source
  field.
- [ ] A named volume retains `source=<name>` and the target.
- [ ] Bind-mount behavior is unchanged and separately covered.
- [ ] A counterfactual test fails with the current always-present source formatting.

## Verification Plan

- Automated checks: focused Feature mount parsing/serialization tests, full tests, lint,
  format, and build; a Docker smoke if unit serialization cannot establish acceptance.
- Manual check: inspect the final Docker `--mount` argv rather than only an internal
  struct.

## Done When

Tests state and enforce the meaningful anonymous-versus-named volume distinction.
