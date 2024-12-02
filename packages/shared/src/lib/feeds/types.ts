import type { ApiUser } from '$lib/users/types';

export type ApiPost = {
	uuid: string;
	content: string;
	post_type: string;
	reply_to_id: string;
	created_at: string;
	user: ApiUser;
	// TODO metadata:
	// TODO target:
};

export type ApiPostWithReplies = ApiPost & {
	replies: ApiPost[];
};

export type Post = {
	uuid: string;
	content: string;
	postType: string;
	replyToId: string;
	createdAt: string;
	userLogin: string;
	// TODO metadata:
	// TODO target:

	replyIds?: string[];
};

export function apiToPost(apiPost: ApiPost): Post {
	return {
		uuid: apiPost.uuid,
		content: apiPost.content,
		postType: apiPost.post_type,
		replyToId: apiPost.reply_to_id,
		createdAt: apiPost.created_at,
		userLogin: apiPost.user.login
	};
}

export type Feed = {
	identifier: string;
	postIds: string[];
};
