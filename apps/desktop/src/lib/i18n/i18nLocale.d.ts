import type { LocaleMessage } from '@intlify/core';

export interface DefineLocaleMessage extends LocaleMessage {
	welcome: {
		auth: {
			title: string;
			description: string;
			loginButton: string;
			cancel: string;
			copyLoginLink: string;
		};
	};
	settings: {
		general: {
			title: string;
			general: {
				label: string;
				signingOut: {
					title: string;
					caption: string;
					button: string;
				};
				language: {
					title: string;
					changeSuccess: string;
				};
				codeEditor: {
					title: string;
				};
				autoUpdate: {
					title: string;
					caption: string;
				};
				cliInstall: {
					title: string;
					captionPackageManager: string;
					captionWindows: string;
					captionUnix: string;
					installButton: string;
					showCommandButton: string;
				};
				removeProjects: {
					title: string;
					caption: string;
					button: string;
					modalTitle: string;
					modalMessage: string;
					removeButton: string;
					cancelButton: string;
					success: string;
					errorFailedDelete: string;
				};
				profileUpdate: {
					fullName: string;
					email: string;
					updateButton: string;
					success: string;
					errorFailedUpdate: string;
					errorInvalidFile: string;
				};
			};
			appearance: {
				label: string;
				theme: {
					title: string;
					light: string;
					dark: string;
					system: string;
				};
				fileListMode: {
					title: string;
					caption: string;
					listView: string;
					treeView: string;
				};
				filePathFirst: {
					title: string;
					caption: string;
				};
				diffPreview: {
					title: string;
				};
				diffFont: {
					title: string;
					caption: string;
				};
				diffLigatures: {
					title: string;
				};
				tabSize: {
					title: string;
					caption: string;
				};
				softWrap: {
					title: string;
					caption: string;
				};
				linesContrast: {
					title: string;
					caption: string;
					light: string;
					medium: string;
					strong: string;
				};
				colorBlindFriendly: {
					title: string;
					caption: string;
				};
				inlineWordDiffs: {
					title: string;
					caption: string;
				};
				scrollbarOnScroll: {
					title: string;
					caption: string;
				};
				scrollbarOnHover: {
					title: string;
					caption: string;
				};
				scrollbarAlways: {
					title: string;
				};
				stagingBehavior: {
					stageAll: {
						title: string;
						caption: string;
					};
					stageSelection: {
						title: string;
						caption: string;
					};
					stageNone: {
						title: string;
						caption: string;
					};
				};
			};
			lanesAndBranches: {
				label: string;
				newLanesPlacement: {
					title: string;
					caption: string;
				};
				autoSelectCreation: {
					title: string;
					caption: string;
				};
				autoSelectRename: {
					title: string;
					caption: string;
				};
			};
			git: {
				label: string;
				committerCredit: {
					title: string;
					caption: string;
				};
				autoFetch: {
					title: string;
					oneMinute: string;
					fiveMinutes: string;
					tenMinutes: string;
					fifteenMinutes: string;
					none: string;
				};
			};
			integrations: {
				label: string;
				autoFillPr: {
					title: string;
					caption: string;
				};
				github: {
					authenticated: string;
					authFailed: string;
					invalidToken: string;
					invalidTokenOrHost: string;
					loadFailed: string;
					tryAgain: string;
					caption: string;
					copyCode: string;
					copyToClipboard: string;
					navigateToGitHub: string;
					openGitHub: string;
					checkStatus: string;
					addPat: string;
					cancel: string;
					addAccount: string;
					addAnotherAccount: string;
					addGhe: string;
					gheCaption: string;
					apiBaseUrl: string;
					apiBaseUrlHelper: string;
					personalAccessToken: string;
					credentialsPersisted: string;
					authorizeAccount: string;
				};
			};
			ai: {
				label: string;
				about: string;
				useButlerApi: string;
				bringYourOwn: string;
				openAi: {
					title: string;
					keyPrompt: string;
					signInMessage: string;
					butlerApiNote: string;
					keyLabel: string;
					modelVersion: string;
					customEndpoint: string;
				};
				anthropic: {
					title: string;
					keyPrompt: string;
					signInMessage: string;
					butlerApiNote: string;
					keyLabel: string;
					modelVersion: string;
				};
				ollama: {
					title: string;
					configTitle: string;
					configContent: string;
				};
				lmStudio: {
					title: string;
					endpoint: string;
					model: string;
					configTitle: string;
					configContent: string;
				};
				contextLength: {
					title: string;
					caption: string;
				};
				customPrompts: {
					title: string;
					description: string;
				};
				modelNames: {
					gpt5: string;
					gpt5Mini: string;
					o3Mini: string;
					o1Mini: string;
					gpt4oMini: string;
					gpt41: string;
					gpt41Mini: string;
					haiku: string;
					sonnet35: string;
					sonnet37: string;
					sonnet4: string;
					opus4: string;
				};
			};
			telemetry: {
				label: string;
				description: string;
				request: string;
				errorReporting: {
					title: string;
					caption: string;
				};
				usageMetrics: {
					title: string;
					caption: string;
				};
				nonAnonMetrics: {
					title: string;
					caption: string;
				};
			};
			experimental: {
				label: string;
				about: string;
				apply3: {
					title: string;
					caption: string;
				};
				fMode: {
					title: string;
					caption: string;
				};
				newRebase: {
					title: string;
					caption: string;
				};
				singleBranch: {
					title: string;
					caption: string;
				};
				irc: {
					title: string;
					caption: string;
					serverLabel: string;
				};
			};
			organizations: {
				label: string;
				createButton: string;
			};
			footer: {
				social: {
					docs: string;
					discord: string;
				};
			};
		};
		project: {
			title: string;
			project: {
				label: string;
			};
			git: {
				label: string;
				allowForcePush: {
					title: string;
					caption: string;
				};
				forcePushProtection: {
					title: string;
					caption: string;
				};
			};
			ai: {
				label: string;
				description: string;
				enableGeneration: {
					title: string;
					caption: string;
				};
				enableExperimental: {
					title: string;
					caption: string;
				};
				customPrompts: {
					title: string;
					description: string;
					button: string;
				};
			};
			agent: {
				label: string;
				guideText: string;
				autoCommit: {
					title: string;
					caption: string;
				};
				useConfiguredModel: {
					title: string;
					caption: string;
				};
				newlineOnEnter: {
					title: string;
					caption: string;
				};
				notifyOnCompletion: string;
				notifyOnPermissionRequest: string;
				dangerousPermissions: {
					title: string;
					caption: string;
				};
			};
			experimental: {
				label: string;
				ignoreCertificate: {
					title: string;
					caption: string;
				};
			};
			details: {
				projectPath: string;
				projectName: string;
				projectNamePlaceholder: string;
				projectDescription: string;
				runGitHooks: {
					title: string;
					caption: string;
				};
			};
			disableCodegen: {
				title: string;
				caption: string;
			};
			baseBranch: {
				loading: string;
				title: string;
				caption: string;
				currentTargetBranch: string;
				createBranchesOnRemote: string;
				activeBranchesWarning: string;
				switchingBranches: string;
				updateConfiguration: string;
				errorListingBranches: string;
			};
			remove: {
				title: string;
				caption: string;
				success: string;
				error: string;
			};
		};
		error: {
			notFound: string;
		};
	};
}
