-- Run once with database/role administration privileges against benchmark database:
--   psql -X -f crates/but/tests/performance/setup-database.sql
-- Uses standard libpq connection settings (PGDATABASE, PGSERVICE, etc.).
-- Prompts securely for benchmark_writer's password after creating schema and role.
\set ON_ERROR_STOP on

BEGIN;

CREATE TABLE public.benchmark_runs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    scenario TEXT NOT NULL,
    commit_sha TEXT NOT NULL, -- revision of benchmarked binary
    version TEXT, -- resolved release version for downloaded binaries; NULL for local binaries
    fixture_sha TEXT NOT NULL,
    harness_sha TEXT NOT NULL, -- revision of benchmark scripts
    machine TEXT NOT NULL, -- stable benchmark host identifier
    os TEXT NOT NULL,
    cpu TEXT NOT NULL,
    warmup_count INTEGER NOT NULL,
    mean_seconds DOUBLE PRECISION NOT NULL,
    stddev_seconds DOUBLE PRECISION,
    median_seconds DOUBLE PRECISION NOT NULL,
    min_seconds DOUBLE PRECISION NOT NULL,
    max_seconds DOUBLE PRECISION NOT NULL,
    hyperfine_json JSONB NOT NULL -- original output for future analysis
);

CREATE TABLE public.benchmark_samples (
    run_id BIGINT NOT NULL REFERENCES public.benchmark_runs(id),
    sample_index INTEGER NOT NULL,
    seconds DOUBLE PRECISION NOT NULL CHECK (seconds >= 0),
    PRIMARY KEY (run_id, sample_index)
);

CREATE INDEX benchmark_runs_history
    ON public.benchmark_runs (scenario, machine, recorded_at DESC);

CREATE ROLE benchmark_writer LOGIN;

-- Grant access to whichever database this script is connected to.
SELECT format('GRANT CONNECT ON DATABASE %I TO benchmark_writer', current_database())
\gexec

GRANT USAGE ON SCHEMA public TO benchmark_writer;
-- Full read access and uploads, but no UPDATE or DELETE privileges.
-- SELECT also permits INSERT ... RETURNING id; identity needs no sequence grant.
GRANT SELECT, INSERT ON TABLE public.benchmark_runs, public.benchmark_samples
    TO benchmark_writer;

COMMIT;

-- No plaintext password in this file or shell history. If interrupted here,
-- run this command separately before using benchmark_writer.
\password benchmark_writer
