import type { ApiUser } from '$lib/users/types';

type BaseApiPost = {
	uuid: string;
	content: string;
	post_type: 'post' | 'commit';
	reply_to_id: string;
	created_at: string;
	user: ApiUser;
	picture_url: string | null;
};

export type ApiRegularPost = BaseApiPost & {
	post_type: 'post';
};

export type ApiCommitPost = BaseApiPost & {
	post_type: 'commit';
	metadata: {
		slug: string;
		branch_id: string;
		oplog_sha: string;
		commit_sha: string;
		original_project: string;
	};
};

export type ApiPost = ApiRegularPost | ApiCommitPost;

export type ApiPostWithReplies = ApiPost & {
	replies: ApiPost[];
};

type PostBase = {
	uuid: string;
	content: string;
	replyToId: string;
	createdAt: string;
	userLogin: string;
	pictureUrl?: string;
	// TODO metadata:
	// TODO target:

	replyIds?: string[];
};

export type RegularPost = PostBase & {
	postType: 'post';
	// TODO target:
};

export type CommitPost = PostBase & {
	postType: 'commit';
	metadata: {
		slug: string;
		branchId: string;
		oplogSha: string;
		commitSha: string;
		originalProject: string;
	};
};

export type Post = RegularPost | CommitPost;

export function apiToPost(apiPost: ApiPost): Post {
	const postBase: PostBase = {
		uuid: apiPost.uuid,
		content: apiPost.content,
		replyToId: apiPost.reply_to_id,
		createdAt: apiPost.created_at,
		userLogin: apiPost.user.login,
		pictureUrl: apiPost.picture_url || undefined
	};

	if (apiPost.post_type === 'commit') {
		return {
			postType: 'commit',
			metadata: {
				slug: apiPost.metadata.slug,
				branchId: apiPost.metadata.branch_id,
				oplogSha: apiPost.metadata.oplog_sha,
				commitSha: apiPost.metadata.commit_sha,
				originalProject: apiPost.metadata.original_project
			},
			...postBase
		};
	} else {
		return {
			postType: 'post',
			...postBase
		};
	}
}

export type Feed = {
	identifier: string;
	postIds: string[];
};
