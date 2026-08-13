import {
	feedbackPrompt,
	type LocalAnnotation,
	type LocalAnnotationsByPath,
	useCommentArchive,
	useCommentUpdate,
} from "#ui/annotation.ts";
import { Annotation } from "#ui/components/Annotation.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import type { FileParent } from "#ui/operands.ts";
import { useHotkey } from "@tanstack/react-hotkeys";
import { useRef, useState, type FC, type RefObject } from "react";

type Props = {
	projectId: string;
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

	// A comment mounted with an empty persisted body is being created. Kept as state rather than
	// derived from the body: blur persists typed text (see persistBody), and deriving would swap
	// Save/Cancel for the saved-comment actions mid-interaction, under the user's focus.
	const [isDraft, setIsDraft] = useState(annotation.body.trim() === "");
	const textareaRef = useRef<HTMLTextAreaElement | null>(null);

	// The last body we sent to the backend, so leaving the form after an explicit Save does not fire
	// a duplicate update.
	const persistedBodyRef = useRef(annotation.body);
	const persistBody = (body: string) => {
		if (body === persistedBodyRef.current) return;
		persistedBodyRef.current = body;
		updateComment({ projectId: p.projectId, id: annotation.id, payload: body });
	};

	const bodyFromForm = (form: HTMLFormElement | null): string => {
		if (!form) throw new Error("Missing owning form");
		const body = new FormData(form).get(annotation.id);
		if (typeof body !== "string") throw new Error("Missing or invalid body");
		return body;
	};

	const archiveAndRefocus = () => {
		// Discard the live edit before moving focus outside the form; otherwise the form-level blur
		// handler would persist it immediately before the archive mutation.
		textareaRef.current?.form?.reset();
		archiveComment({ projectId: p.projectId, id: annotation.id });
		p.selectionScopeRef.current?.focus({ focusVisible: false });
	};

	const saveAndRefocus = (body: string) => {
		persistBody(body);
		setIsDraft(false);
		p.selectionScopeRef.current?.focus({ focusVisible: false });
	};

	const copyAll = () => {
		const forms = p.selectionScopeRef.current?.querySelectorAll<HTMLFormElement>(
			"form[data-local-annotation]",
		);

		const mountedBodies: Map<string, string> = new Map(
			forms
				?.values()
				.flatMap((form) =>
					new FormData(form)
						.entries()
						.flatMap(([id, body]) => (typeof body === "string" ? [[id, body]] : [])),
				),
		);

		const feedback = p.annotationsByPath.entries().flatMap(([path, annotations]) =>
			annotations.map((annotation) => ({
				annotation: {
					...annotation,
					// Use any live mounted bodies, falling back to persisted.
					body: mountedBodies.get(annotation.id) ?? annotation.body,
				},
				fileParent: p.fileParent,
				path,
			})),
		);

		void window.lite.clipboardWriteText(feedbackPrompt(feedback.toArray()));
	};

	useHotkey("Mod+Enter", () => textareaRef.current?.form?.requestSubmit(), {
		conflictBehavior: "allow",
		ignoreInputs: false,
		target: textareaRef,
	});

	useHotkey(
		"Escape",
		() => {
			const form = textareaRef.current?.form;
			if (form && bodyFromForm(form).trim() === "") archiveAndRefocus();
		},
		{
			conflictBehavior: "allow",
			enabled: isDraft,
			ignoreInputs: false,
			target: textareaRef,
		},
	);

	return (
		// oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- Equivalent to a blur event on each input.
		<form
			data-local-annotation
			onBlur={(evt) => {
				const next = evt.relatedTarget;
				// Moving between the textarea and its actions has not left this annotation.
				if (next instanceof Node && evt.currentTarget.contains(next)) return;
				// Pierre gracefully blurs focused annotations before virtualizing their item,
				// permitting uncontrolled input state to be persisted.
				persistBody(bodyFromForm(evt.currentTarget));
			}}
			onSubmit={(evt) => {
				evt.preventDefault();
				saveAndRefocus(bodyFromForm(evt.currentTarget));
			}}
		>
			<Annotation
				author="You"
				defaultBody={annotation.body}
				updatedAt={annotation.updatedAtMs}
				name={annotation.id}
				textareaRef={(textarea) => {
					textareaRef.current = textarea;
					if (textarea && focusAnnotationIdRef.current === annotation.id) {
						focusAnnotationIdRef.current = null;
						textarea.focus();
					}
				}}
				actions={
					isDraft ? (
						<>
							<button type="submit" className={getButtonClassName({ variant: "pop" })}>
								Save
							</button>

							<button
								type="button"
								className={getButtonClassName({ variant: "ghost" })}
								onClick={archiveAndRefocus}
							>
								Cancel
							</button>
						</>
					) : (
						<>
							<button
								type="button"
								className={getButtonClassName({ variant: "ghost" })}
								onClick={archiveAndRefocus}
							>
								Archive
							</button>

							<button
								type="button"
								aria-label="Copy as prompt"
								title="Copy as prompt"
								style={{ marginLeft: "auto" }}
								className={getButtonClassName({ variant: "ghost", iconOnly: true })}
								onClick={(evt) => {
									const body = bodyFromForm(evt.currentTarget.form);
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
								<Icon name="copy" />
							</button>

							<button
								type="button"
								aria-label="Copy all as prompt"
								title="Copy all as prompt"
								className={getButtonClassName({ variant: "ghost", iconOnly: true })}
								onClick={copyAll}
							>
								<Icon name="checklist" />
							</button>
						</>
					)
				}
			/>
		</form>
	);
};
