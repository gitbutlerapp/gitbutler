import { useQueryClient, useSuspenseQueries } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FC } from "react";
import {
	bitbucketAccountsQueryOptions,
	githubAccountsQueryOptions,
	gitlabAccountsQueryOptions,
} from "#ui/api/queries.ts";
import {
	useForgetBitbucketAccount,
	useForgetGithubAccount,
	useForgetGitlabAccount,
	useStoreBitbucketApiToken,
	useStoreGithubPat,
	useStoreGitlabPat,
} from "#ui/api/mutations.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Logo, type LogoName } from "#ui/components/Logo.tsx";
import { classes } from "#ui/components/classes.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import { nativeMenuItem, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { signInWithGithub } from "./github-oauth.ts";
import styles from "./Integrations.module.css";

/** One account as the card shows it, over whatever identifier its forge uses. */
type Account<Identifier> = {
	username: string;
	/** How it authenticated — the part a username alone hides. */
	kind: string;
	identifier: Identifier;
};

type ForgeCardProps<Identifier> = {
	name: string;
	logo: LogoName;
	blurb: string;
	accounts: Array<Account<Identifier>>;
	/** Bitbucket wants an email alongside the token; the others do not. */
	needsEmail?: boolean;
	/** Present when the forge supports signing in through the browser. */
	onSignIn?: () => void;
	/** Shown once the browser flow has a code for the user to enter. */
	pendingCode?: string | null;
	isBusy: boolean;
	onForget: (account: Identifier) => void;
	onAdd: (token: string, email: string) => void;
};

const ForgeCard = <Identifier,>(p: ForgeCardProps<Identifier>) => {
	const [adding, setAdding] = useState(false);
	const [token, setToken] = useState("");
	const [email, setEmail] = useState("");

	// Bitbucket names the account by the email its token was issued for, so a blank one
	// would post a request it cannot fulfil.
	const incomplete = token.trim() === "" || (p.needsEmail === true && email.trim() === "");

	const submit = () => {
		if (incomplete) return;
		p.onAdd(token, email);
		setToken("");
		setEmail("");
		setAdding(false);
	};

	return (
		<section className={styles.card}>
			{p.accounts.map((account) => (
				// Keyed on the identifier because that is what the forge treats as unique: a
				// username repeats across auth kinds, and across enterprise hosts.
				<div key={JSON.stringify(account.identifier)} className={styles.account}>
					<span className={styles.identity}>
						<span className="text-13 text-semibold">{account.username}</span>
						<span className={classes("text-12", styles.muted)}>{account.kind}</span>
					</span>
					<button
						type="button"
						className={classes(
							getButtonClassName({ variant: "danger", size: "small" }),
							styles.addButton,
						)}
						disabled={p.isBusy}
						onClick={() => p.onForget(account.identifier)}
					>
						Forget
						<Icon name="bin" size={12} />
					</button>
				</div>
			))}

			<div className={classes(styles.forge, p.accounts.length > 0 && styles.forgeUnderAccounts)}>
				<Logo name={p.logo} className={styles.forgeLogo} />
				<span className={styles.identity}>
					<span className="text-13 text-semibold">{p.name}</span>
					<span className={classes("text-12", styles.muted)}>{p.blurb}</span>
				</span>

				{!adding && (
					<button
						type="button"
						className={classes(getButtonClassName({ size: "small" }), styles.addButton)}
						disabled={p.isBusy}
						// One way in goes straight there; several offer the choice, as desktop does.
						onClick={(event) => {
							if (p.onSignIn === undefined) return setAdding(true);
							void showNativeMenuFromTrigger(event.currentTarget, [
								nativeMenuItem({ label: `Authorize ${p.name} Account`, onSelect: p.onSignIn }),
								nativeMenuItem({
									label: "Add Personal Access Token",
									onSelect: () => setAdding(true),
								}),
							]);
						}}
					>
						{p.accounts.length > 0 ? "Add another account" : "Add account"}
						<Icon name="plus" size={12} />
					</button>
				)}
			</div>

			{p.pendingCode != null && (
				<p className={classes("text-12", styles.deviceCode)}>
					Enter <code>{p.pendingCode}</code> on the GitHub page that just opened. This waits until
					you have.
				</p>
			)}

			{adding && (
				<form
					className={styles.addForm}
					onSubmit={(evt) => {
						evt.preventDefault();
						submit();
					}}
				>
					{p.needsEmail === true && (
						<input
							type="email"
							required
							className="text-13"
							placeholder="Account email"
							value={email}
							onChange={(evt) => setEmail(evt.currentTarget.value)}
						/>
					)}
					<input
						type="password"
						className="text-13"
						placeholder="Personal access token"
						autoComplete="off"
						value={token}
						onChange={(evt) => setToken(evt.currentTarget.value)}
					/>
					<div className={styles.addActions}>
						<button
							type="submit"
							className={getButtonClassName({ variant: "pop", size: "small" })}
							disabled={p.isBusy || incomplete}
						>
							{p.isBusy ? "Adding…" : "Add"}
						</button>
						<button
							type="button"
							className={getButtonClassName({ size: "small" })}
							onClick={() => {
								setAdding(false);
								setToken("");
								setEmail("");
							}}
						>
							Cancel
						</button>
					</div>
				</form>
			)}
		</section>
	);
};

const githubKind = (type: string): string =>
	type === "oAuthUsername" ? "OAuth" : type === "enterprise" ? "Enterprise" : "Access token";

export const Integrations: FC = () => {
	const [{ data: github }, { data: gitlab }, { data: bitbucket }] = useSuspenseQueries({
		queries: [
			githubAccountsQueryOptions,
			gitlabAccountsQueryOptions,
			bitbucketAccountsQueryOptions,
		],
	});

	const forgetGithub = useForgetGithubAccount();
	const forgetGitlab = useForgetGitlabAccount();
	const forgetBitbucket = useForgetBitbucketAccount();
	const addGithub = useStoreGithubPat();
	const addGitlab = useStoreGitlabPat();
	const addBitbucket = useStoreBitbucketApiToken();
	const client = useQueryClient();

	const [githubCode, setGithubCode] = useState<string | null>(null);
	const [githubBusy, setGithubBusy] = useState(false);
	const [githubError, setGithubError] = useState<string | null>(null);
	const inFlight = useRef<AbortController | null>(null);

	// Closing the dialog abandons the device flow, so the poll should go with it.
	useEffect(() => () => inFlight.current?.abort(), []);

	const signInGithub = () => {
		const controller = new AbortController();
		inFlight.current = controller;
		setGithubBusy(true);
		setGithubError(null);
		signInWithGithub({
			client,
			signal: controller.signal,
			onCode: (flow) => setGithubCode(flow.userCode),
		})
			// Abandoning is not a failure to report.
			.catch((error: unknown) => {
				if (!controller.signal.aborted) setGithubError(errorMessageForToast(error));
			})
			.finally(() => {
				if (controller.signal.aborted) return;
				setGithubBusy(false);
				setGithubCode(null);
			});
	};

	return (
		<>
			<ForgeCard
				name="GitHub"
				logo="github"
				blurb="Allows you to create Pull Requests"
				accounts={github.map((account) => ({
					username: account.info.username,
					kind: githubKind(account.type),
					identifier: account,
				}))}
				isBusy={forgetGithub.isPending || addGithub.isPending || githubBusy}
				onSignIn={signInGithub}
				pendingCode={githubCode}
				onForget={forgetGithub.mutate}
				onAdd={(token) => addGithub.mutate(token)}
			/>

			<ForgeCard
				name="GitLab"
				logo="gitlab"
				blurb="Allows you to create Merge Requests"
				accounts={gitlab.map((account) => ({
					username: account.info.username,
					kind: account.type === "selfHosted" ? "Self-hosted" : "Access token",
					identifier: account,
				}))}
				isBusy={forgetGitlab.isPending || addGitlab.isPending}
				onForget={forgetGitlab.mutate}
				onAdd={(token) => addGitlab.mutate(token)}
			/>

			<ForgeCard
				name="Bitbucket"
				logo="bitbucket"
				blurb="Allows you to create Pull Requests"
				needsEmail
				accounts={bitbucket.map((account) => ({
					// Bitbucket names an account by the email its token was issued for.
					username: account.info.email,
					kind: "API token",
					identifier: account,
				}))}
				isBusy={forgetBitbucket.isPending || addBitbucket.isPending}
				onForget={forgetBitbucket.mutate}
				onAdd={(accessToken, email) => addBitbucket.mutate({ email, accessToken })}
			/>

			{githubError !== null && <p className={classes("text-12", styles.error)}>{githubError}</p>}

			<p className={classes("text-12", styles.footnote)}>
				<Icon name="lock" className={styles.footnoteIcon} />
				Credentials are kept in your operating system's keychain, not by GitButler.
			</p>
		</>
	);
};
