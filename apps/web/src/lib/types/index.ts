export interface TimelineEntry {
	sha: string;
	target_tree_sha: string;
	time: string;
	message: string;
	trailers?: string;
	// TODO: branch_data
	// vb_blob = lookup(commit.tree['virtual_branches.toml'][:oid]).content
	// PerfectTOML.parse(vb_blob)
	branch_data?: {
		branches: {
			[key: string]: unknown;
		};
	};
	files?: {
		[key: string]: PatchFile;
	};
}

export interface ActiveStorageAttachment {
	id: number;
	name: string;
	record_type: string;
	record_id: number;
	blob_id: number;
	created_at: string;
}

export interface ActiveStorageBlob {
	id: number;
	key: string;
	filename: string;
	content_type?: string;
	metadata?: string;
	service_name: string;
	byte_size: number;
	checksum?: string;
	created_at: string;
}

export interface ActiveStorageVariantRecord {
	id: number;
	blob_id: number;
	variation_digest: string;
}

export interface Butlog {
	id: number;
	loggable_type?: string;
	loggable_id?: number;
	user_id?: number;
	level?: string;
	message?: string;
	tags?: Record<string, any>;
	meta?: Record<string, any>;
	created_at: string;
	updated_at: string;
}

export interface ChatMessage {
	id: number;
	project_id: number;
	chattable_id?: string;
	user_id?: number;
	outdated: boolean;
	issue: boolean;
	resolved: boolean;
	uuid?: string;
	thread_id?: string;
	comment?: Record<string, any>;
	data?: Record<string, any>;
	created_at: string;
	updated_at: string;
}

export interface Chat {
	id: number;
	user_id: number;
	chat_id?: string;
	created_at: string;
	updated_at: string;
}

export interface ClientProject {
	id: number;
	client_id?: number;
	project_id?: number;
	created_at: string;
	updated_at: string;
}

export interface Client {
	id: number;
	user_id?: number;
	name?: string;
	client_id?: string;
	created_at: string;
	updated_at: string;
}

export interface CodeShare {
	id: number;
	user_id?: number;
	project_id?: number;
	code: string;
	path?: string;
	line_start?: number;
	line_end?: number;
	session_commit_sha?: string;
	blob_sha?: string;
	created_at: string;
	updated_at: string;
}

export interface DiffCache {
	id: number;
	project_id?: number;
	diff_sha: string;
	new_file_sha: string;
	base_file_sha: string;
	diff_patch?: string;
	hunks?: number;
	lines?: number;
	additions?: number;
	deletions?: number;
	new_size?: number;
	old_size?: number;
	created_at: string;
	updated_at: string;
}

export interface Feedback {
	id: number;
	user_id?: number;
	feedback?: string;
	context?: string;
	created_at: string;
	updated_at: string;
	email?: string;
}

export interface LoginToken {
	id: number;
	token: string;
	ip_from: string;
	expires_at: string;
	redeemed_at?: string;
	user_id?: number;
	created_at: string;
	updated_at: string;
}

export interface MergeSessionFile {
	id: number;
	merge_session_id: number;
	path?: string;
	diff?: string;
	diff_type?: string;
	status?: string;
	resolution?: string;
	chat?: Record<string, any>;
	created_at: string;
	updated_at: string;
}

export interface MergeSession {
	id: number;
	user_id: number;
	project_id?: number;
	status?: string;
	created_at: string;
	updated_at: string;
}

export interface Message {
	id: number;
	chat_id: number;
	role: number;
	content: string;
	created_at: string;
	updated_at: string;
}

export interface OrganizationProject {
	id: number;
	organization_id: number;
	name?: string;
	description?: string;
	slug?: string;
	trunk_url?: string;
	trunk_branch?: string;
	created_at: string;
	updated_at: string;
}

export interface OrganizationUser {
	id: number;
	organization_id?: number;
	user_id?: number;
	role?: string;
}

export interface Organization {
	id: number;
	name?: string;
	slug?: string;
	description?: string;
	created_at: string;
	updated_at: string;
	invite_code?: string;
}

export interface PatchEvent {
	id: number;
	project_id?: number;
	user_id?: number;
	uuid?: string;
	change_id?: string;
	event_type?: string;
	eventable_type?: string;
	eventable_id?: number;
	data?: Record<string, any>;
	created_at: string;
	updated_at: string;
}

export interface PatchFile {
	id: number;
	patch_id: number;
	old_path?: string;
	new_path?: string;
	position?: number;
	created_at: string;
	updated_at: string;
	diff_cache_id?: number;
}

export interface PatchSection {
	id: number;
	patch_id: number;
	version?: number;
	type?: string;
	code?: string;
	data?: Record<string, any>;
	position?: number;
	created_at: string;
	updated_at: string;
}

export interface PatchStack {
	branch_id: string;
	oplog_sha?: string;
	uuid?: string;
	title?: string;
	description?: string;
	status?: string;
	version?: number;
	stack_size?: number;
	contributors?: string[];
	patches: Patch[];
	created_at: string;
	updated_at: string;
}

export interface PatchUserStatus {
	id: number;
	patch_stack_id: number;
	change_id: string;
	user_id?: number;
	last_viewed?: string;
	last_reviewed?: string;
	sign_off?: boolean;
	patch_id?: number;
	created_at: string;
	updated_at: string;
}

export interface Patch {
	id: number;
	patch_stack_id: number;
	commit_sha: string;
	change_id: string;
	title?: string;
	description?: string;
	contributors?: string;
	position?: number;
	version?: number;
	created_at: string;
	updated_at: string;
	patch_sha?: string;
	original_commit_sha?: string;
}

export interface Project {
	id: number;
	name?: string;
	description?: string;
	repository_id?: string;
	user_id?: number;
	created_at: string;
	updated_at: string;
	last_push_at?: string;
	last_fetch_at?: string;
	uid?: string;
	directory?: string;
	code_repository_id?: string;
	last_code_fetch_at?: string;
	last_code_push_at?: string;
	organization_project_id?: number;
	share_level?: string;
	slug?: string;
	last_visit_at?: string;
	ec2_key_name?: string;
}

export interface ReleaseBuild {
	id: number;
	release_id?: number;
	os?: string;
	arch?: string;
	url?: string;
	signature?: string;
	file?: string;
}

export interface Release {
	id: number;
	version?: string;
	notes?: string;
	built_at?: string;
	status?: string;
	build_version?: string;
	published_at?: string;
	sha?: string;
	channel: string;
}

export interface TeamOrganizationProject {
	id: number;
	team_id?: number;
	organization_project_id?: number;
}

export interface TeamUser {
	id: number;
	team_id?: number;
	user_id?: number;
}

export interface Team {
	id: number;
	name?: string;
	description?: string;
	organization_id: number;
	created_at: string;
	updated_at: string;
}

export interface UserEvent {
	id: number;
	user_id: number;
	event_type: string;
	data: Record<string, any>;
	created_at: string;
	updated_at: string;
}

export interface UserNotification {
	id: number;
	user_id: number;
	notification_id?: string;
	body?: string;
	created_at: string;
	updated_at: string;
}

export interface User {
	id: number;
	auth0_id?: string;
	name?: string;
	given_name?: string;
	family_name?: string;
	email?: string;
	login?: string;
	email_verified: boolean;
	picture?: string;
	locale?: string;
	access_token?: string;
	created_at: string;
	updated_at: string;
	role?: string;
	end_of_day_hour: number;
	current_tz_offset: number;
	last_active?: string;
	supporter: boolean;
	stripe_customer_id?: string;
	meta?: Record<string, any>;
	notifications?: Record<string, any>;
	last_client_active_at?: string;
	intercom_id?: string;
	oauth_id?: string;
	oauth_provider?: string;
	password_digest?: string;
	auth_type: string;
}

export interface Version {
	id: number;
	item_type: string;
	item_id: number;
	event: string;
	whodunnit?: string;
	object?: string;
	created_at?: string;
}
