import { Button } from "#ui/components/ui/button.tsx";
import { getHowStatus, howStatusQueryKey } from "#ui/lib/how-status-query.ts";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { CodingAgent, HowStatus } from "../../../electron/src/ipc";

const minimumSaveDelaySeconds = 1;
const maximumSaveDelaySeconds = 60;

const initialStatus: HowStatus = {
	project: null,
	saveState: "idle",
	message: null,
	lastSavedAt: null,
	checkpoints: [],
	bookmarks: [],
	browsing: null,
	settings: {
		checkpointDebounceMs: 10_000,
		codingAgent: "none",
		fetchIntervalMs: 15 * 60 * 1000,
	},
	sharedProject: {
		state: "unknown",
		lastCheckedAt: null,
		message: null,
	},
};

const fetchIntervalOptions = [
	{ label: "Off", value: 0 },
	{ label: "5 min", value: 5 * 60 * 1000 },
	{ label: "15 min", value: 15 * 60 * 1000 },
	{ label: "30 min", value: 30 * 60 * 1000 },
	{ label: "60 min", value: 60 * 60 * 1000 },
] as const;

function clampSaveDelaySeconds(value: number): number {
	if (!Number.isFinite(value)) return 10;
	if (value < minimumSaveDelaySeconds) return minimumSaveDelaySeconds;
	if (value > maximumSaveDelaySeconds) return maximumSaveDelaySeconds;
	return Math.round(value);
}

export function HowSettings() {
	const queryClient = useQueryClient();
	const statusQuery = useQuery({
		queryKey: howStatusQueryKey,
		queryFn: getHowStatus,
		placeholderData: initialStatus,
	});
	const status = statusQuery.data ?? initialStatus;
	const setStatus = useCallback(
		(nextStatus: HowStatus | ((currentStatus: HowStatus) => HowStatus)) => {
			queryClient.setQueryData<HowStatus>(howStatusQueryKey, (currentStatus) => {
				if (typeof nextStatus === "function") return nextStatus(currentStatus ?? initialStatus);
				return nextStatus;
			});
		},
		[queryClient],
	);
	const [saveDelaySeconds, setSaveDelaySeconds] = useState("10");
	const [codingAgent, setCodingAgent] = useState<CodingAgent>("none");
	const [fetchIntervalMs, setFetchIntervalMs] = useState(String(15 * 60 * 1000));
	const [busy, setBusy] = useState(false);
	const [message, setMessage] = useState<string | null>(null);

	useEffect(() => {
		const unsubscribe = window.how.onStatus((nextStatus) => {
			setStatus(nextStatus);
		});
		return () => {
			unsubscribe();
		};
	}, [setStatus]);

	useEffect(() => {
		setSaveDelaySeconds(String(status.settings.checkpointDebounceMs / 1000));
		setCodingAgent(status.settings.codingAgent);
		setFetchIntervalMs(String(status.settings.fetchIntervalMs));
	}, [
		status.project?.id,
		status.settings.checkpointDebounceMs,
		status.settings.codingAgent,
		status.settings.fetchIntervalMs,
	]);

	async function saveSettings() {
		const normalizedSeconds = clampSaveDelaySeconds(Number(saveDelaySeconds));
		setBusy(true);
		setMessage(null);
		try {
			const nextStatus = await window.how.saveProjectSettings({
				checkpointDebounceMs: normalizedSeconds * 1000,
				codingAgent,
				fetchIntervalMs: Number(fetchIntervalMs),
			});
			setStatus(nextStatus);
			setSaveDelaySeconds(String(nextStatus.settings.checkpointDebounceMs / 1000));
			setCodingAgent(nextStatus.settings.codingAgent);
			setFetchIntervalMs(String(nextStatus.settings.fetchIntervalMs));
			setMessage("Saved");
		} catch {
			setMessage("How could not save settings.");
		} finally {
			setBusy(false);
		}
	}

	if (!status.project)
		return (
			<main className="min-h-screen px-6 py-6">
				<div className="mx-auto flex w-full max-w-3xl flex-col gap-6">
					<Button asChild variant="ghost" size="sm" className="self-start">
						<Link to="/">
							<ArrowLeft className="h-4 w-4" aria-hidden />
							Back
						</Link>
					</Button>
					<p className="text-sm text-stone-600">Open or start a project first.</p>
				</div>
			</main>
		);

	return (
		<main className="min-h-screen px-6 py-6">
			<div className="mx-auto flex w-full max-w-3xl flex-col gap-8">
				<header className="flex items-center justify-between gap-4">
					<div className="flex min-w-0 items-center gap-3">
						<Button asChild variant="ghost" size="icon" aria-label="Back">
							<Link to="/">
								<ArrowLeft className="h-4 w-4" aria-hidden />
							</Link>
						</Button>
						<div className="min-w-0">
							<h1 className="truncate text-xl font-semibold tracking-normal text-stone-800">
								Project settings
							</h1>
						</div>
					</div>
				</header>

				<section className="space-y-6">
					<label className="block">
						<span className="text-sm font-medium text-stone-950">Save delay</span>
						<span className="mt-1 block text-sm text-stone-500">
							How saves after this many quiet seconds.
						</span>
						<div className="mt-3 flex items-center gap-3">
							<input
								className="h-10 w-24 rounded-md border border-stone-300 bg-white px-3 text-sm text-stone-950 outline-none focus:border-stone-900 focus:ring-2 focus:ring-stone-200"
								type="number"
								min={minimumSaveDelaySeconds}
								max={maximumSaveDelaySeconds}
								step={1}
								value={saveDelaySeconds}
								onChange={(event) => setSaveDelaySeconds(event.currentTarget.value)}
							/>
							<span className="text-sm text-stone-500">seconds</span>
						</div>
					</label>

					<fieldset>
						<legend className="text-sm font-medium text-stone-950">Coding agent</legend>
						<div className="mt-3 flex flex-wrap gap-2">
							{(["none", "codex", "claude"] as const).map((agent) => (
								<label
									key={agent}
									className={`inline-flex h-10 cursor-pointer items-center rounded-md border px-4 text-sm font-medium ${
										codingAgent === agent
											? "border-stone-900 bg-stone-950 text-white"
											: "border-stone-300 bg-white text-stone-700"
									}`}
								>
									<input
										className="sr-only"
										type="radio"
										name="coding-agent"
										value={agent}
										checked={codingAgent === agent}
										onChange={() => setCodingAgent(agent)}
									/>
									{agent === "none" ? "None" : agent === "codex" ? "Codex" : "Claude"}
								</label>
							))}
						</div>
					</fieldset>

					<label className="block">
						<span className="text-sm font-medium text-stone-950">Check for shared updates</span>
						<span className="mt-1 block text-sm text-stone-500">
							How checks quietly without moving your files.
						</span>
						<select
							className="mt-3 h-10 rounded-md border border-stone-300 bg-white px-3 text-sm text-stone-950 outline-none focus:border-stone-900 focus:ring-2 focus:ring-stone-200"
							value={fetchIntervalMs}
							onChange={(event) => setFetchIntervalMs(event.currentTarget.value)}
						>
							{fetchIntervalOptions.map((option) => (
								<option key={option.value} value={option.value}>
									{option.label}
								</option>
							))}
						</select>
					</label>
				</section>

				<div className="flex items-center gap-3">
					<Button onClick={() => void saveSettings()} disabled={busy}>
						{busy ? "Saving" : "Save"}
					</Button>
					{message ? <p className="text-sm text-stone-600">{message}</p> : null}
				</div>
			</div>
		</main>
	);
}
