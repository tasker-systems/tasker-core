# Tenets

The short list everything else follows from. Each is stated once here; the doctrine files apply it.
Sources: tasker-core's eleven tenets (`docs/principles/tasker-core-tenets.md`) and the principles
temper enforces through its tripwires and outcome registers. Where both said the same thing in
different words, the shared form is kept.

1. **Enforced or explained.** A written rule names the gate that enforces it, or says why it is
   only a convention. Guidance that claims more than the gates enforce is a defect, the same as a
   bug.

2. **A gate that cannot fail is decorative.** Every gate has a test that proves it still goes red
   on the thing it guards against. No blocking check is allowed to swallow its own failure.

3. **Fail loudly, fail closed.** A missing required value is an error, not a default. A scan that
   finds nothing to check fails rather than passing vacuously. An unknown input is refused, not
   guessed at.

4. **Audit before enforce.** A new rule starts by recording the current state as a reviewed
   baseline, then ratchets. It never starts with a blanket exemption, and it never starts by
   breaking every build at once.

5. **Derived over hand-kept.** A list a human maintains drifts. Generate it from the source of
   truth, or check the hand-kept one against it.

6. **A suppression is a reviewed exception.** Silencing a lint, an advisory, a test or an unsafe
   check carries a reason, and the set of exceptions is visible in one place.

7. **Bounded resources.** Every queue, channel and pool has a configured bound and a declared
   strategy for what happens at the bound.

8. **Failure is heard.** A concurrent unit of work that panics or errors is observed by something
   that can act on it. Nothing dies silently in the background.

9. **The database is the coordination layer.** State that must be consistent across processes is
   decided by atomic database primitives; application checks are an additional layer, never the
   only one.

10. **Nothing moves underneath a release.** Every input to CI and to a shipped artifact is pinned
    to an immutable reference, or held behind a cooldown, or both.

11. **Credentials are a type, not a string.** They cannot be printed, compared in variable time,
    or logged, and the first of those is proven mechanically.

12. **Never weaken a test to get green.** Removing an assertion, ignoring a test or loosening a
    gate to make a failure go away hides the problem. Fix the cause, or register the exception
    with its reason.
