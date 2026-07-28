import {
	feedbackPrompt,
	type LocalAnnotation,
	type LocalAnnotationsByPath,
	useCommentArchive,
	useCommentUpdate,
} from "#ui/annotation.ts";
import { Annotation } from "#ui/components/Annotation.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import type { FileParent } from "#ui/operands.ts";
import type { FC, RefObject } from "react";

type Props = {
	projectId: string;
	formId: string;
	annotation: LocalAnnotation;
	path: string;
	fileParent: FileParent;
	/** All annotations of the current file parent, for "Copy all as prompt". */
	annotationsByPath: LocalAnnotationsByPath;
	/** Set to an annotation id to focus its textarea on mount (freshly created comments). */
	focusAnnotationIdRef: RefObject<string | null>;
	selectionScopeRef: RefObject<HTMLDivElement | null>;
};

/** One backend-persisted comment rendered inline in the diff, with its edit and action surface. */
export const AnnotationCard: FC<Props> = (p) => {
	const { annotation, focusAnnotationIdRef } = p;
	const { mutate: updateComment } = useCommentUpdate();
	const { mutate: archiveComment } = useCommentArchive();

	return (
		<Annotation
			author="You"
			defaultBody={annotation.body}
			updatedAt={annotation.updatedAtMs}
			formId={p.formId}
			name={annotation.id}
			textareaRef={(textarea) => {
				if (textarea && focusAnnotationIdRef.current === annotation.id) {
					focusAnnotationIdRef.current = null;
					textarea.focus();
				}
			}}
			// Pierre gracefully blurs focused annotations before virtualizing their item,
			// permitting uncontrolled input state to be persisted.
			onBlur={(body) => {
				// A comment that never got any text is an abandoned gutter click:
				// archive it (cancel) rather than leaving a blank box around forever.
				if (body.trim() === "" && annotation.body.trim() === "")
					archiveComment({ projectId: p.projectId, id: annotation.id });
				else if (body !== annotation.body)
					updateComment({ projectId: p.projectId, id: annotation.id, payload: body });
			}}
			actions={
				<>
					<button
						type="button"
						className={getButtonClassName({ variant: "ghost" })}
						onClick={() => {
							archiveComment({ projectId: p.projectId, id: annotation.id });
							p.selectionScopeRef.current?.focus({ focusVisible: false });
						}}
					>
						Archive
					</button>

					<button
						type="button"
						form={p.formId}
						className={getButtonClassName({ variant: "ghost" })}
						onClick={(evt) => {
							const form = evt.currentTarget.form;
							if (!form) throw new Error("Missing owning form");

							const body = new FormData(form).get(annotation.id);
							if (typeof body !== "string") throw new Error("Missing or invalid body");

							void window.lite.clipboardWriteText(
								feedbackPrompt([
									{
										annotation: { ...annotation, body },
										fileParent: p.fileParent,
										path: p.path,
									},
								]),
							);
						}}
					>
						Copy as prompt
					</button>

					<button
						type="button"
						form={p.formId}
						className={getButtonClassName({ variant: "ghost" })}
						onClick={(evt) => {
							const form = evt.currentTarget.form;
							if (!form) throw new Error("Missing owning form");

							const formData = new FormData(form);
							const feedback = p.annotationsByPath
								.entries()
								.flatMap(([path, annotations]) =>
									annotations.map((annotation) => {
										// Use any live mounted bodies, falling back to persisted.
										const formBody = formData.get(annotation.id);
										return {
											annotation: {
												...annotation,
												body: typeof formBody === "string" ? formBody : annotation.body,
											},
											fileParent: p.fileParent,
											path,
										};
									}),
								)
								.toArray();

							void window.lite.clipboardWriteText(feedbackPrompt(feedback));
						}}
					>
						Copy all as prompt
					</button>
				</>
			}
		/>
	);
};
