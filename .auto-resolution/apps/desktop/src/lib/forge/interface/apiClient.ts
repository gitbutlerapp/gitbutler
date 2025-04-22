export interface ApiClient {
	// Called when the client has initialized, or reset, and any data must
	// be refetched. This might be the case because the auth token has
	// changed, or because you have switched projects and the redux cache
	// must be cleared.
	onReset(fn: () => void): () => void;
}
