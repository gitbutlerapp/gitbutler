import { z } from 'zod';

export const ChangeStateSchema = z.object({
	id: z.string(),
	kind: z.string()
});

export const FlagsSchema = z.enum([
	'ExecutableBitAdded',
	'ExecutableBitRemoved',
	'TypeChangeFileToLink',
	'TypeChangeLinkToFile',
	'TypeChange'
]);

export const AdditionSchema = z.object({
	state: ChangeStateSchema,
	isUntracked: z.boolean()
});

export const DeletionSchema = z.object({
	previousState: ChangeStateSchema
});

export const ModificationSchema = z.object({
	previousState: ChangeStateSchema,
	state: ChangeStateSchema,
	flags: FlagsSchema.nullable()
});

export const RenameSchema = z.object({
	previousPath: z.string(),
	previousState: ChangeStateSchema,
	state: ChangeStateSchema,
	flags: FlagsSchema.nullable()
});

export const IgnoredChangeStatusSchema = z.enum(['Conflict', 'TreeIndex']);

export const IgnoredChangeSchema = z.object({
	path: z.string(),
	status: IgnoredChangeStatusSchema
});

export const StatusSchema = z.discriminatedUnion('type', [
	z.object({ type: z.literal('Addition'), subject: AdditionSchema }),
	z.object({ type: z.literal('Deletion'), subject: DeletionSchema }),
	z.object({ type: z.literal('Modification'), subject: ModificationSchema }),
	z.object({ type: z.literal('Rename'), subject: RenameSchema })
]);

export const TreeChangeSchema = z.object({
	path: z.string(),
	pathBytes: z.array(z.number()),
	status: StatusSchema
});

export const TooLargeSchema = z.object({
	sizeInBytes: z.number()
});

export const DiffHunkSchema = z.object({
	oldStart: z.number(),
	oldLines: z.number(),
	newStart: z.number(),
	newLines: z.number(),
	diff: z.string()
});

export const PatchSchema = z.object({
	hunks: z.array(DiffHunkSchema),
	isResultOfBinaryToTextConversion: z.boolean(),
	linesAdded: z.number().optional(),
	linesRemoved: z.number().optional()
});

export const UnifiedDiffSchema = z.discriminatedUnion('type', [
	z.object({ type: z.literal('Binary') }),
	z.object({ type: z.literal('TooLarge'), subject: TooLargeSchema }),
	z.object({ type: z.literal('Patch'), subject: PatchSchema })
]);

export const ChangeUnifiedDiffSchema = z.object({
	diff: UnifiedDiffSchema,
	treeChange: TreeChangeSchema
});

export const UnifiedWorktreeChanges = z.object({
	changes: z.array(ChangeUnifiedDiffSchema),
	ignoredChanges: z.array(IgnoredChangeSchema)
});
