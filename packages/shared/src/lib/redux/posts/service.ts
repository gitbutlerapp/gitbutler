import { InterestStore } from '$lib/redux/interest';
import { upsertFeed, upsertPost, upsertPosts } from '$lib/redux/posts/slice';
import { apiToPost, type ApiPost, type Post } from '$lib/redux/posts/types';
import type { HttpClient } from '$lib/httpClient';
import type { AppDispatch } from '$lib/redux/store';

export class FeedService {
	private readonly feedInterests = new InterestStore<undefined>(5 * 60 * 1000);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly dispatch: AppDispatch
	) {}

	/** Used to start querying and polling the feed */
	getFeedInterest() {
		return this.feedInterests.createInterest(undefined, async () => {
			const apiFeed = await this.httpClient.get<ApiPost[]>('feed');
			this.dispatch(upsertPosts(apiFeed.map(apiToPost)));
			this.dispatch(upsertFeed({ identifier: 'all', postIds: apiFeed.map((post) => post.uuid) }));
		});
	}

	async createPost(content: string): Promise<Post> {
		const apiPost = await this.httpClient.post<ApiPost>('post', { body: { content } });
		const post = apiToPost(apiPost);
		this.dispatch(upsertPost(post));

		// TODO: Determine if this is needed / wanted / useful
		const apiFeed = await this.httpClient.get<ApiPost[]>('feed');
		this.dispatch(upsertPosts(apiFeed.map(apiToPost)));
		this.dispatch(upsertFeed({ identifier: 'all', postIds: apiFeed.map((post) => post.uuid) }));

		return post;
	}
}
