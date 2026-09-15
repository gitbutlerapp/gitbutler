import { useQuery } from "@tanstack/react-query";
import { useInstallCli } from "#ui/api/mutations.ts";
import { isPackagedQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Row } from "./Section.tsx";
import { Toast } from "@base-ui/react";

export const InstallCli = () => {
	const { data: isPackaged } = useQuery(isPackagedQueryOptions);
	const { mutate, isPending, data: installed } = useInstallCli();
	const toastManager = Toast.useToastManager();

	if (window.lite.platform !== "darwin" || !isPackaged) return null;

	let buttonLabel = "Install";
	if (isPending) buttonLabel = "Installing ...";
	else if (installed) buttonLabel = "but is Installed";

	return (
		<Row
			label="Command-line interface"
			hint="Makes but available at /usr/local/bin/but. Administrator authorization may be required."
		>
			<div>
				<button
					type="button"
					className={getButtonClassName({ size: "small" })}
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
				</button>
			</div>
		</Row>
	);
};
