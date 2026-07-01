/**
 * Module-level log sink for but-server output.
 * Safe because tests run sequentially within each worker process.
 * Cleared at the start of each test by the _autoArtifacts fixture.
 */
export const serverLogSink: string[] = [];
