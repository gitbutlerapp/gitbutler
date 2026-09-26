import { useApplyReview } from "#ui/api/mutations.ts";
import {
	forgeAccountsQueryOptions,
	forgeInfoOptions,
	getReviewQueryOptions,
} from "#ui/api/queries.ts";
import { encodeBytes } from "#ui/api/bytes.ts";
import { branchAddress } from "#ui/addresses.ts";
import { forgeDestination, isCloudForge } from "#ui/forge.ts";
import { setCursor, setPage } from "#ui/use-cursor.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import {
	FieldControlStyles,
	FieldLabelStyles,
	FieldRootStyles,
} from "@gitbutler/ui-react/Field.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { Modal, ModalBody, ModalFooter, ModalHeader } from "@gitbutler/ui-react/Popup.tsx";
import { Field, Toolbar } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type FC, type SyntheticEvent, useRef, useState } from "react";
import { getRowButtonClassName } from "./Row-utils.ts";
import styles from "./ApplyPullRequest.module.css";

/** Digits only, without a leading zero, and small enough to stay exact as a number. */
const parsePullRequestNumber = (input: string): number | null => {
	const trimmed = input.trim();
	if (!/^[1-9][0-9]*$/.test(trimmed)) return null;
	const number = Number(trimmed);
	return Number.isSafeInteger(number) ? number : null;
};

const ApplyPullRequestModal: FC<{ projectId: string; onClose: () => void }> = ({
	projectId,
	onClose,
}) => {
	const inputRef = useRef<HTMLInputElement>(null);
	const [input, setInput] = useState("");
	const [invalid, setInvalid] = useState(false);
	// Only a submitted number is previewed, so an edit drops the preview it no longer matches.
	const [submitted, setSubmitted] = useState<number | null>(null);
	const preview = useQuery({
		...getReviewQueryOptions({ projectId, reviewId: submitted ?? 0 }),
		enabled: submitted !== null,
	});
	const review = submitted === null ? undefined : preview.data;
	const applyReview = useApplyReview(projectId);

	const submit = (event: SyntheticEvent<HTMLFormElement>) => {
		event.preventDefault();
		if (preview.isFetching || applyReview.isPending) return;

		if (review) {
			applyReview.mutate(review.number, {
				onSuccess: (outcome) => {
					// Applied outside a workspace, the checked-out branch comes first and the pull request's last.
					const appliedRef =
						outcome.status === "applied" ? outcome.appliedBranches.at(-1) : undefined;
					if (appliedRef) {
						onClose();
						setPage("workspace");
						setCursor("applied", branchAddress({ branchRef: encodeBytes(appliedRef.full) }));
					} else if (outcome.status === "alreadyApplied") {
						onClose();
					}
				},
			});
			return;
		}

		const number = parsePullRequestNumber(input);
		setInvalid(number === null);
		if (number === null) return;
		if (number === submitted) void preview.refetch();
		else setSubmitted(number);
	};

	return (
		<Modal
			open
			initialFocus={inputRef}
			onOpenChange={(open) => {
				if (!open && !applyReview.isPending) onClose();
			}}
		>
			<form onSubmit={submit}>
				<ModalHeader
					title="Apply pull request"
					description="Adds its branch to your workspace, even when it comes from a fork."
				/>
				<ModalBody>
					<Field.Root invalid={invalid} render={<FieldRootStyles />}>
						<Field.Label render={<FieldLabelStyles />}>Pull request number</Field.Label>
						<Field.Control
							ref={inputRef}
							render={<FieldControlStyles />}
							autoComplete="off"
							inputMode="numeric"
							placeholder="42"
							value={input}
							disabled={applyReview.isPending}
							onValueChange={(value) => {
								setInput(value);
								setInvalid(false);
								setSubmitted(null);
							}}
						/>
						{invalid && (
							<Field.Error match className={classes("text-12", styles.error)}>
								Enter the number only, like 42.
							</Field.Error>
						)}
					</Field.Root>
					{review ? (
						<div>
							<p className="text-13 text-semibold">{review.title}</p>
							<p className={classes("text-12", styles.meta)}>
								{review.unitSymbol}
								{review.number} · {review.sourceBranch}
							</p>
						</div>
					) : preview.isFetching ? (
						<p className={classes("text-13", styles.meta)}>Loading…</p>
					) : (
						submitted !== null &&
						preview.isError && (
							<p className={classes("text-13", styles.error)}>Could not load the pull request.</p>
						)
					)}
				</ModalBody>
				<ModalFooter>
					<Button variant="ghost" disabled={applyReview.isPending} onClick={onClose}>
						Cancel
					</Button>
					<Button
						type="submit"
						variant="gray"
						disabled={preview.isFetching || applyReview.isPending}
					>
						{review ? (applyReview.isPending ? "Applying…" : "Apply to workspace") : "Continue"}
					</Button>
				</ModalFooter>
			</form>
		</Modal>
	);
};

/** Applies a pull request by its number, offered where a GitHub account can fetch one. */
export const ApplyPullRequest: FC<{ projectId: string }> = ({ projectId }) => {
	const [open, setOpen] = useState(false);
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const destination = forgeDestination(forgeInfo);
	const { data: hasAccount } = useQuery({
		...forgeAccountsQueryOptions(destination?.name),
		select: (accounts) => destination !== null && isCloudForge(destination) && accounts.length > 0,
	});
	if (
		destination?.name !== "github" ||
		forgeInfo?.capabilities.prService !== true ||
		hasAccount !== true
	)
		return null;

	return (
		<>
			<Toolbar.Button
				aria-label="Apply pull request"
				className={getRowButtonClassName({ size: "regular", iconOnly: true })}
				onClick={() => setOpen(true)}
			>
				<Icon name="pr" />
			</Toolbar.Button>
			{open && <ApplyPullRequestModal projectId={projectId} onClose={() => setOpen(false)} />}
		</>
	);
};
