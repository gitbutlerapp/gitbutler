import { getWorkspaceItemRowButtonClassName } from "../WorkspaceItemRow-utils.ts";
import { WorkspaceItemRowLabel, WorkspaceItemRowLabelContainer } from "../WorkspaceItemRow.tsx";
import { formatForDisplaySorted } from "#ui/hotkeys.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { useHotkey } from "@tanstack/react-hotkeys";
import { FC, useId, useRef } from "react";
import styles from "./InlineEditor.module.css";

export const InlineEditor: FC<{
	value: string;
	label: string;
	onMount?: (el: HTMLTextAreaElement | HTMLInputElement) => void;
	onSubmit: (value: string) => void;
	onExit: () => void;
	multiline: boolean;
	heading?: boolean;
}> = ({ value, label, onMount, onSubmit, onExit, multiline, heading }) => {
	const name = useId();
	const formRef = useRef<HTMLFormElement | null>(null);
	const textFieldRef = useRef<HTMLTextAreaElement | HTMLInputElement>(null);
	const submitAction = (formData: FormData) => {
		onExit();
		onSubmit(formData.get(name) as string);
	};

	useHotkey("Enter", () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		ignoreInputs: false,
		target: textFieldRef,
	});

	useHotkey("Escape", onExit, {
		conflictBehavior: "allow",
		ignoreInputs: false,
		target: textFieldRef,
	});

	const allTextFieldRefs = useMergedRefs(textFieldRef, (el) => {
		if (!el) return;
		el.focus();
		onMount?.(el);
	});

	return (
		<form ref={formRef} className={styles.form} action={submitAction}>
			<WorkspaceItemRowLabelContainer>
				<WorkspaceItemRowLabel
					heading={heading}
					aria-label={label}
					className={styles.input}
					render={
						multiline ? (
							<textarea ref={allTextFieldRefs} name={name} defaultValue={value} />
						) : (
							<input ref={allTextFieldRefs} name={name} defaultValue={value} />
						)
					}
				/>
			</WorkspaceItemRowLabelContainer>
			<div className={styles.help}>
				<button type="submit" className={getWorkspaceItemRowButtonClassName({})}>
					<kbd>{formatForDisplaySorted("Enter")}</kbd>
					<span className={styles.shortcutLabel}> to Save</span>
				</button>
				<button type="button" className={getWorkspaceItemRowButtonClassName({})} onClick={onExit}>
					<kbd>{formatForDisplaySorted("Escape")}</kbd>
					<span className={styles.shortcutLabel}> to Cancel</span>
				</button>
			</div>
		</form>
	);
};
