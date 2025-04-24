export type ApiUpload = {
	uuid: string;
	filename: string;
	content_type: string;
	url: string;
	public: boolean;
	created_at: string;
};

export type Upload = {
	uuid: string;
	filename: string;
	contentType: string;
	url: string;
	public: boolean;
	createdAt: string;
	isImage: boolean;
};

export function isImage(contentType: string): boolean {
	return contentType.startsWith('image/');
}

export function apiToUpload(apiUpload: ApiUpload): Upload {
	return {
		uuid: apiUpload.uuid,
		filename: apiUpload.filename,
		contentType: apiUpload.content_type,
		url: apiUpload.url,
		public: apiUpload.public,
		createdAt: apiUpload.created_at,
		isImage: isImage(apiUpload.content_type)
	};
}
