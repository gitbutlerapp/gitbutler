export type ApiPost = {
	uuid: string;
	content: string;
	post_type: string;
	reply_to_id: string;
	// TODO user:
	// TODO metadata:
	// TODO target:
};

export type Post = {
	uuid: string;
	content: string;
	postType: string;
	replyToId: string;
	// TODO userId:
	// TODO metadata:
	// TODO target:
};

export function apiToPost(apiPost: ApiPost): Post {
	return {
		uuid: apiPost.uuid,
		content: apiPost.content,
		postType: apiPost.post_type,
		replyToId: apiPost.reply_to_id
	};
}

export type Feed = {
	identifier: string;
	postIds: string[];
};
