import { Field } from "@base-ui/react";
import { useMutation, useQuery, useQueryClient, useSuspenseQueries } from "@tanstack/react-query";
import { useEffect, useRef, useState, type FC, type ReactNode } from "react";
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
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import {
	FieldControlStyles,
	FieldLabelStyles,
	FieldRootStyles,
} from "@gitbutler/ui-react/Field.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { Logo, type LogoName } from "@gitbutler/ui-react/Logo.tsx";
import { TextLink } from "@gitbutler/ui-react/TextLink.tsx";
import { errorMessageForToast } from "#ui/errors.ts";
import { nativeMenuItem, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { useCopied } from "#ui/components/useCopied.ts";
import { openLinkExternally } from "#ui/external-link.ts";
import {
	butDevLoginQueryOptions,
	butDevSessionQueryOptions,
	disconnectButDev,
	startButDevLogin,
} from "#ui/but-dev/auth.ts";
import { signInWithGithub } from "./github-oauth.ts";
import styles from "./Integrations.module.css";
import { Note, Section } from "./Section.tsx";

const IntegrationRow: FC<{
	mark: ReactNode;
	title: string;
	hint: string;
	children: ReactNode;
}> = ({ mark, title, hint, children }) => (
	<div className={styles.row}>
		<div className={styles.logo}>{mark}</div>
		<div className={styles.text}>
			<span className={classes("text-15", "text-semibold", styles.title)}>{title}</span>
			<span className={classes("text-12", "text-body", styles.hint)}>{hint}</span>
		</div>
		{children}
	</div>
);

const ButDev: FC<{ heading?: string }> = ({ heading }) => {
	const client = useQueryClient();
	const {
		data: session,
		isPending: loading,
		error: sessionError,
		refetch,
	} = useQuery(butDevSessionQueryOptions);
	const {
		mutate: openBrowser,
		error: browserError,
		reset: resetBrowser,
	} = useMutation({ mutationFn: (url: string) => window.lite.openInWebBrowser(url) });
	const {
		data: approval,
		mutate: connect,
		isPending: starting,
		error: connectError,
		reset: resetConnection,
	} = useMutation({ mutationFn: startButDevLogin });
	const { error: pollError } = useQuery(butDevLoginQueryOptions(approval));
	const {
		data: disconnectNotice,
		mutate: disconnect,
		isPending: disconnecting,
		error: disconnectError,
		reset: resetDisconnect,
	} = useMutation({ mutationFn: () => disconnectButDev(client) });

	const waiting = approval !== undefined && !session && !pollError;
	const error =
		sessionError ??
		disconnectError ??
		(!session ? (connectError ?? pollError ?? browserError) : null);
	const notice = error?.message ?? disconnectNotice;

	return (
		<>
			<Section
				heading={heading}
				footer={
					waiting ? (
						<>
							<p className={classes("text-12", "text-body", styles.deviceCode)}>
								Approve code <strong>{approval.user_code}</strong> in your browser.
							</p>
							<Button onClick={() => openBrowser(approval.verify_url)}>Open browser</Button>
						</>
					) : session ? (
						<p className={classes("text-12", "text-body", styles.deviceCode)}>
							Connected as {session.login}
						</p>
					) : undefined
				}
			>
				<IntegrationRow
					mark={<Icon name="globe" />}
					title="but.dev"
					hint="Uses your GitButler account with a separate token for now; we plan to unify sign-in."
				>
					{session ? (
						<Button
							disabled={disconnecting}
							onClick={() => {
								resetConnection();
								resetBrowser();
								disconnect();
							}}
						>
							{disconnecting ? "Disconnecting…" : "Disconnect"}
						</Button>
					) : starting || waiting ? (
						<Button
							onClick={() => {
								void client.cancelQueries({ queryKey: ["butDev", "login"] });
								resetConnection();
								resetBrowser();
							}}
						>
							Cancel
						</Button>
					) : sessionError ? (
						<Button onClick={() => void refetch()}>Retry</Button>
					) : (
						<Button
							disabled={loading || disconnecting}
							onClick={() => {
								resetDisconnect();
								resetBrowser();
								// Per-call callbacks stop on reset/unmount, so a cancelled start opens no browser.
								connect(undefined, {
									onSuccess: (flow) => openBrowser(flow.verify_url),
								});
							}}
						>
							Connect
						</Button>
					)}
				</IntegrationRow>
			</Section>
			{notice != null && <Note icon="warning">{notice}</Note>}
		</>
	);
};

/** The code GitHub's device flow wants typed into the page it opened, with a button to carry it. */
const DeviceCode: FC<{ code: string }> = (p) => {
	const { copied, copy } = useCopied(p.code);
	return (
		<>
			<p className={classes("text-12", "text-body", styles.deviceCode)}>
				Enter <strong>{p.code}</strong> on the GitHub page that just opened. <br /> This waits until
				you have.
			</p>
			<Button variant="gray" onClick={copy}>
				{copied ? "Copied" : "Copy code"}
				<Icon name={copied ? "tick" : "copy"} />
			</Button>
		</>
	);
};

type ForgeCardProps = {
	/** Only the first card names the group the cards make up. */
	heading?: string;
	name: string;
	logo: LogoName;
	blurb: string;
	/** What a valid token looks like, since the user may not know. */
	tokenPlaceholder: string;
	/** The scope the token needs and where to generate one. */
	tokenHint: ReactNode;
	/** Bitbucket wants an email alongside the token; the others do not. */
	needsEmail?: boolean;
	/** Present when the forge supports signing in through the browser. */
	onSignIn?: () => void;
	/** Shown once the browser flow has a code for the user to enter. */
	pendingCode?: string | null;
	isBusy: boolean;
	onAdd: (token: string, email: string) => void;
};

const ForgeCard: FC<ForgeCardProps> = (p) => {
	const [adding, setAdding] = useState(false);
	const [token, setToken] = useState("");
	const [email, setEmail] = useState("");

	// Bitbucket names the account by the email its token was issued for, so a blank one
	// would post a request it cannot fulfil.
	const incomplete = token.trim() === "" || (p.needsEmail === true && email.trim() === "");

	const close = () => {
		setAdding(false);
		setToken("");
		setEmail("");
	};

	const submit = () => {
		if (incomplete) return;
		p.onAdd(token, email);
		close();
	};

	const signIn = p.onSignIn;

	const footer =
		p.pendingCode != null ? (
			<DeviceCode code={p.pendingCode} />
		) : adding ? (
			<form
				className={styles.form}
				onSubmit={(evt) => {
					evt.preventDefault();
					submit();
				}}
			>
				<div className={styles.fields}>
					{p.needsEmail === true && (
						<Field.Root render={<FieldRootStyles />}>
							<Field.Label render={<FieldLabelStyles />}>Account email</Field.Label>
							<Field.Control
								render={<FieldControlStyles />}
								type="email"
								required
								value={email}
								onValueChange={(value) => setEmail(value)}
							/>
						</Field.Root>
					)}
					<Field.Root render={<FieldRootStyles />}>
						<Field.Label render={<FieldLabelStyles />}>Personal access token</Field.Label>
						<Field.Control
							render={<FieldControlStyles />}
							type="password"
							autoComplete="off"
							placeholder={p.tokenPlaceholder}
							value={token}
							onValueChange={(value) => setToken(value)}
						/>
					</Field.Root>
					<p className={classes("text-12", "text-body", styles.hint)}>{p.tokenHint}</p>
				</div>
				<div className={styles.actions}>
					<Button type="submit" variant="gray" disabled={p.isBusy || incomplete}>
						{p.isBusy ? "Authorizing…" : "Authorize"}
						<Icon name="tick" />
					</Button>
					<Button onClick={close}>Cancel</Button>
				</div>
			</form>
		) : undefined;

	return (
		<Section heading={p.heading} footer={footer}>
			<IntegrationRow mark={<Logo name={p.logo} muted />} title={p.name} hint={p.blurb}>
				<Button
					disabled={p.isBusy}
					// One way in goes straight there; several offer the choice, as desktop does.
					onClick={(event) => {
						if (signIn === undefined) return setAdding(true);
						void showNativeMenuFromTrigger(event.currentTarget, [
							nativeMenuItem({
								label: `Authorize ${p.name} Account`,
								onSelect: () => {
									close();
									signIn();
								},
							}),
							nativeMenuItem({
								label: "Add Personal Access Token",
								onSelect: () => setAdding(true),
							}),
						]);
					}}
				>
					Connect
				</Button>
			</IntegrationRow>
		</Section>
	);
};

/** One connected account, whichever forge it belongs to. */
type ConnectedAccount = {
	key: string;
	logo: LogoName;
	username: string;
	/** The forge and how it authenticated — the part a username alone hides. */
	kind: string;
	isBusy: boolean;
	onForget: () => void;
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

	// Keyed on the identifier because that is what the forge treats as unique: a username
	// repeats across auth kinds, and across enterprise hosts.
	const connected: Array<ConnectedAccount> = [
		...github.map((account) => ({
			key: `github:${JSON.stringify(account)}`,
			logo: "github" as const,
			username: account.info.username,
			kind:
				account.type === "enterprise"
					? `GitHub · Enterprise · ${account.info.host}`
					: `GitHub · ${githubKind(account.type)}`,
			isBusy: forgetGithub.isPending,
			onForget: () => forgetGithub.mutate(account),
		})),
		...gitlab.map((account) => ({
			key: `gitlab:${JSON.stringify(account)}`,
			logo: "gitlab" as const,
			username: account.info.username,
			kind:
				account.type === "selfHosted"
					? `GitLab · Self-hosted · ${account.info.host}`
					: "GitLab · Access token",
			isBusy: forgetGitlab.isPending,
			onForget: () => forgetGitlab.mutate(account),
		})),
		...bitbucket.map((account) => ({
			key: `bitbucket:${JSON.stringify(account)}`,
			logo: "bitbucket" as const,
			// Bitbucket names an account by the email its token was issued for.
			username: account.info.email,
			kind: "Bitbucket · API token",
			isBusy: forgetBitbucket.isPending,
			onForget: () => forgetBitbucket.mutate(account),
		})),
	];

	return (
		<>
			{connected.length > 0 && (
				<Section heading="Connected">
					{connected.map((account) => (
						<IntegrationRow
							key={account.key}
							mark={<Logo name={account.logo} />}
							title={account.username}
							hint={account.kind}
						>
							<Button variant="danger" disabled={account.isBusy} onClick={account.onForget}>
								Forget
							</Button>
						</IntegrationRow>
					))}
				</Section>
			)}

			<ButDev heading="Add an account" />

			<ForgeCard
				name="GitHub"
				logo="github"
				blurb="Create and review pull requests"
				tokenPlaceholder="ghp_XXXXXXXXXXXXXXXXXXXX"
				tokenHint={
					<>
						Classic token with the repo scope, or a fine-grained token with Pull requests: read and
						write, plus Checks: read for CI status.{" "}
						<TextLink href="https://github.com/settings/tokens" onClick={openLinkExternally}>
							Generate on GitHub
						</TextLink>
					</>
				}
				isBusy={forgetGithub.isPending || addGithub.isPending || githubBusy}
				onSignIn={signInGithub}
				pendingCode={githubCode}
				onAdd={(token) => addGithub.mutate(token)}
			/>

			<ForgeCard
				name="GitLab"
				logo="gitlab"
				blurb="Create and review merge requests"
				tokenPlaceholder="glpat-XXXXXXXXXXXXXXXXXXXX"
				tokenHint={
					<>
						Token with the api scope.{" "}
						<TextLink
							href="https://gitlab.com/-/user_settings/personal_access_tokens"
							onClick={openLinkExternally}
						>
							Generate on GitLab
						</TextLink>
					</>
				}
				isBusy={forgetGitlab.isPending || addGitlab.isPending}
				onAdd={(token) => addGitlab.mutate(token)}
			/>

			<ForgeCard
				name="Bitbucket"
				logo="bitbucket"
				blurb="Create and review pull requests"
				tokenPlaceholder="ATATT3xXXXXXXXXXXXXXXXXXXXXX"
				tokenHint={
					<>
						API token with the read:user:bitbucket, read:repository:bitbucket,
						read:pullrequest:bitbucket and write:pullrequest:bitbucket scopes, plus the email on
						your Atlassian account.{" "}
						<TextLink
							href="https://id.atlassian.com/manage-profile/security/api-tokens"
							onClick={openLinkExternally}
						>
							Generate on Atlassian
						</TextLink>
					</>
				}
				needsEmail
				isBusy={forgetBitbucket.isPending || addBitbucket.isPending}
				onAdd={(accessToken, email) => addBitbucket.mutate({ email, accessToken })}
			/>

			{githubError !== null && <p className={classes("text-12", styles.error)}>{githubError}</p>}

			<Note icon="lock">
				Forge credentials use your operating system's keychain. The but.dev token is stored locally
				in this app.
			</Note>
		</>
	);
};
