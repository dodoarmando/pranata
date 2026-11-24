-- A claimed job is leased, not locked for the whole run: the claim commits at
-- once (counting the attempt), and the job is not offered again until the
-- lease ends. A worker that crashes mid-job therefore leaves a job that has
-- already been counted and that becomes claimable again after the lease.
ALTER TABLE jobs ADD COLUMN locked_until timestamptz;
