# Root service group correction

The reported live installation remains pending; this code/test change did not perform rollback, installation or finalization on the host.

Cause: SYSTEM inherited admin group. copy creates a destination inode in that directory and does not preserve ownership. Setting stage ownership alone would therefore be insufficient. Old tests checked script bytes, mode and plist fields; root metadata was simulated for old backups, but newly copied destination GID was not asserted. That gap allowed root:admin scripts through isolated installation tests until strict runtime verification rejected them.

Fix: after copying each new or restored fixed service file, validate its final destination through one O_NOFOLLOW descriptor, require regular single-link uid 0 exact bytes and mode, then fchown(0,0) and fsync when GID differs. Compare directory-entry dev/ino before and after correction. Strict watchdog verification is unchanged.

Pending health: finalize/commit loads the protected journal and verifies components first. It validates all expected fixed root service files before correcting their GIDs. No App replacement or new stage/backup is performed. Public verify stays read-only. Unsafe bytes/modes/UIDs/symlinks/hardlinks reject and leave the journal pending.

Evidence: red-tests.txt records 2 expected assertion failures. green-tests.txt and full-packaging-tests.txt record 97 passing tests. py_compile and git diff --check succeeded. New tests mock root metadata and chown; no real root ownership, launchctl, sudo, network, app or account operations occurred. frozen-sha256.txt freezes the two changed source files for independent review.
