export type ApiPost = {
	uuid: string;
	content: string;
	post_type: string;
	reply_to_id: string;
	created_at: string;
	// TODO user:
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
	// TODO userId:
	// TODO metadata:
	// TODO target:
};

export function apiToPost(apiPost: ApiPost): Post {
	return {
		uuid: apiPost.uuid,
		content: apiPost.content,
		postType: apiPost.post_type,
		replyToId: apiPost.reply_to_id,
		createdAt: apiPost.created_at
	};
}

export type Feed = {
	identifier: string;
	postIds: string[];
};

export type PostReplies = {
	postId: string;
	replyIds: string[];
};
