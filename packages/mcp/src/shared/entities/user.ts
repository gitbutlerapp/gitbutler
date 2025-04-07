import { z } from 'zod';

export const UserSimpleSchema = z.object({
	owner_type: z.enum(['user', 'organization'], {
		description: 'The type of the owner of the user'
	}),
	id: z.number({ description: 'The ID of the user' }).min(1),
	login: z.string({ description: 'The username of the user' }).nullable(),
	name: z.string({ description: 'The name of the user' }).nullable(),
	email: z.string({ description: 'The email of the user' }).nullable(),
	avatar_url: z.string({ description: 'The avatar URL of the user' }).nullable()
});

export type UserSimple = z.infer<typeof UserSimpleSchema>;

export const UserSchema = UserSimpleSchema.extend({
	created_at: z.string({ description: 'The time the user was created' }).optional(),
	updated_at: z.string({ description: 'The time the user was updated' }).optional(),
	website: z.string({ description: 'The website of the user' }).nullable(),
	twitter: z.string({ description: 'The Twitter handle of the user' }).nullable(),
	bluesky: z.string({ description: 'The BlueSky handle of the user' }).nullable()
});

export type User = z.infer<typeof UserSchema>;

export const UserMaybeSchema = z.object({
	email: z.string({ description: 'The email of the user' }).nullable(),
	user: UserSimpleSchema.nullable().optional()
});

export type UserMaybe = z.infer<typeof UserMaybeSchema>;
