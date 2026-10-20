# vm-broker-increments-k38

**Reviews:** vm-broker-increments-k4

## Goal

Read the planned subtree of `vm-broker-increments-k4` with fresh context. Find the defects that would make an implementation session fail or build the wrong thing. Report findings and fix nothing.

## Context

- The subtree is every node and leaf from `agent-client-hyper-k7` to `release-publish-k37`, and the root brief.
- The design is `docs/specs/testanyware-app.md` and the ADRs 0004, 0019, 0020 and 0021.
- The owner asked for small increments and no rabbit holes. Agents under herdr are blocked today, so an early unblock matters.

## Done when

- Each finding names the leaf or node, the defect and the session that it would hurt.
- The review checks that each increment leaves the product working and has a test or a live check that proves it.
- The review checks that the dependency order holds, and that no requirement of the spec lacks a leaf.
- The review checks that each leaf fits one session.
