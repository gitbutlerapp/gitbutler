-- Called by run.sh with psql variables; quoted interpolation protects SQL values.
-- One statement makes run and sample insertion atomic.
WITH input AS (
    SELECT :'hyperfine_json'::jsonb AS document
), inserted AS (
    INSERT INTO public.benchmark_runs (
        scenario, commit_sha, version, fixture_sha, harness_sha, machine, os, cpu,
        warmup_count, mean_seconds, stddev_seconds, median_seconds,
        min_seconds, max_seconds, hyperfine_json
    )
    SELECT
        :'scenario', :'commit_sha', NULLIF(:'version', ''), :'fixture_sha', :'harness_sha',
        :'machine', :'os', :'cpu', :'warmup_count'::integer,
        (document #>> '{results,0,mean}')::double precision,
        (document #>> '{results,0,stddev}')::double precision,
        (document #>> '{results,0,median}')::double precision,
        (document #>> '{results,0,min}')::double precision,
        (document #>> '{results,0,max}')::double precision,
        document
    FROM input
    RETURNING id
)
INSERT INTO public.benchmark_samples (run_id, sample_index, seconds)
SELECT inserted.id, (sample.ordinality - 1)::integer, sample.value::double precision
FROM inserted, input,
    jsonb_array_elements_text(input.document #> '{results,0,times}')
        WITH ORDINALITY AS sample(value, ordinality);
