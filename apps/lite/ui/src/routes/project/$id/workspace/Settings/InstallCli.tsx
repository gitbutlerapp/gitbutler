import { useQuery } from "@tanstack/react-query";
import { useInstallCli } from "#ui/api/mutations.ts";
import { isPackagedQueryOptions } from "#ui/api/queries.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { Illustration } from "@gitbutler/ui-react/Illustration.tsx";
import { Row, Section } from "./Section.tsx";
import { Toast } from "@base-ui/react";

export const InstallCli = () => {
	const { data: isPackaged } = useQuery(isPackagedQueryOptions);
	const { mutate, isPending, data: installed } = useInstallCli();
	const toastManager = Toast.useToastManager();

	if (window.lite.platform !== "darwin" || !isPackaged) return null;

	let buttonLabel = "Install";
	if (isPending) buttonLabel = "Installing…";
	else if (installed) buttonLabel = "Installed";

	return (
		// A card of its own: the drawing leads, and nothing else on the page is about the CLI.
		<Section>
			<Row
				label="Command-line interface"
				leading={<Illustration name="terminal" />}
				hint={
					<>
						Makes but available at /usr/local/bin/but.
						<br />
						Administrator authorization may be required.
					</>
				}
			>
				<Button
					disabled={isPending || installed}
					onClick={() =>
						mutate(undefined, {
							onSuccess: (installed) => {
								if (!installed) return;
								toastManager.add({
									type: "success",
									title: "but is installed",
									description:
										"The but CLI is now installed! Run `but --help` in a terminal for usage.",
									priority: "low",
								});
							},
						})
					}
				>
					{buttonLabel}
					<Icon name={isPending ? "spinner" : installed ? "tick" : "arrow-in-box"} />
				</Button>
			</Row>
		</Section>
	);
};
