import type { DefineLocaleMessage } from '$lib/i18n/i18nLocale';

const locale: DefineLocaleMessage = {
	welcome: {
		auth: {
			title: 'Log in or Sign up',
			description:
				'Log in to access smart automation features, including intelligent branch creation and commit message generation.',
			loginButton: 'Log in / Sign up',
			cancel: 'Cancel',
			copyLoginLink: 'Copy login link'
		}
	},
	settings: {
		general: {
			title: 'Global settings',
			general: {
				label: 'General',
				signingOut: {
					title: 'Signing out',
					caption: 'Ready to take a break? Click here to log out and unwind.',
					button: 'Log out'
				},
				language: {
					title: 'Language',
					changeSuccess: 'Language changed successfully'
				},
				codeEditor: {
					title: 'Default code editor'
				},
				autoUpdate: {
					title: 'Automatically check for updates',
					caption: 'Automatically check for updates. You can still check manually when needed.'
				},
				cliInstall: {
					title: 'Install the GitButler CLI <code class="code-string">but</code>',
					captionPackageManager:
						'The <code>but</code> CLI is managed by your package manager. Please use your package manager to install, update, or remove it.',
					captionWindows:
						'On Windows, you can manually copy the executable (<code>`but`</code>) to a directory in your PATH. Click "Show Command" for instructions.',
					captionUnix:
						'Installs the GitButler CLI (<code>`but`</code>) in your PATH, allowing you to use it from the terminal. This action will request admin privileges. Alternatively, you could create a symlink manually.',
					installButton: 'Install But CLI',
					showCommandButton: 'Show command'
				},
				removeProjects: {
					title: 'Remove all projects',
					caption:
						'You can delete all projects from the GitButler app.<br />Your code remains safe. it only clears the configuration.',
					button: 'Remove projects…',
					modalTitle: 'Remove all projects',
					modalMessage: 'Are you sure you want to remove all GitButler projects?',
					removeButton: 'Remove',
					cancelButton: 'Cancel',
					success: 'All data deleted',
					errorFailedDelete: 'Failed to delete project'
				},
				profileUpdate: {
					fullName: 'Full name',
					email: 'Email',
					updateButton: 'Update profile',
					success: 'Profile updated',
					errorFailedUpdate: 'Failed to update user',
					errorInvalidFile: 'Please use a valid image file'
				}
			},
			appearance: {
				label: 'Appearance',
				theme: {
					title: 'Theme',
					light: 'Light',
					dark: 'Dark',
					system: 'System preference'
				},
				fileListMode: {
					title: 'Default file list mode',
					caption: 'Set the default file list view (can be changed per location).',
					listView: 'List view',
					treeView: 'Tree view'
				},
				filePathFirst: {
					title: 'File path first',
					caption: 'Display the full file path before the file name in file lists.'
				},
				diffPreview: {
					title: 'Diff preview'
				},
				diffFont: {
					title: 'Font family',
					caption:
						'Sets the font for the diff view. The first font name is the default, others are fallbacks.'
				},
				diffLigatures: {
					title: 'Allow font ligatures'
				},
				tabSize: {
					title: 'Tab size',
					caption: 'Number of spaces per tab in the diff view.'
				},
				softWrap: {
					title: 'Soft wrap',
					caption: 'Soft wrap long lines in the diff view to fit within the viewport.'
				},
				linesContrast: {
					title: 'Lines contrast',
					caption: 'The contrast for added, deleted, and context lines in diffs.',
					light: 'Light',
					medium: 'Medium',
					strong: 'Strong'
				},
				colorBlindFriendly: {
					title: 'Color blind-friendly colors',
					caption:
						'Use blue and orange colors instead of green and red for better<br />accessibility with color vision deficiency.'
				},
				inlineWordDiffs: {
					title: 'Display word diffs inline',
					caption:
						'Instead of separate lines for removals and additions, this feature shows a single line with both added and removed words highlighted.'
				},
				scrollbarOnScroll: {
					title: 'Scrollbar-On-Scroll',
					caption: 'Only show the scrollbar when you are scrolling.'
				},
				scrollbarOnHover: {
					title: 'Scrollbar-On-Hover',
					caption: 'Show the scrollbar only when you hover over the scrollable area.'
				},
				scrollbarAlways: {
					title: 'Always show scrollbar'
				},
				stagingBehavior: {
					stageAll: {
						title: 'Stage all files',
						caption:
							'Stage all files assigned to the stack on commit. If no files are staged, all unassigned files will be staged.'
					},
					stageSelection: {
						title: 'Stage selected files',
						caption:
							'Stage the selected assigned files to the stack on commit. If no files are selected, stage all files. If there are no assigned files, stage all selected unassigned files.<br />And if no files are selected, stage all unassigned files.'
					},
					stageNone: {
						title: "Don't stage files automatically",
						caption:
							"Do not stage any files automatically.<br />You're more of a DIY developer in that way."
					}
				}
			},
			lanesAndBranches: {
				label: 'Lanes & branches',
				newLanesPlacement: {
					title: 'Place new lanes on the left side',
					caption:
						'By default, new lanes are added to the rightmost position. Enable this to add them to the leftmost position instead.'
				},
				autoSelectCreation: {
					title: 'Auto-select text on branch creation',
					caption:
						'Automatically select the pre-populated text in the branch name field when creating a new branch, making it easier to type your own name.'
				},
				autoSelectRename: {
					title: 'Auto-select text on branch rename',
					caption:
						'Automatically select the text when renaming a branch or lane, making it easier to replace the entire name.'
				}
			},
			git: {
				label: 'Git stuff',
				committerCredit: {
					title: 'Credit GitButler as the committer',
					caption:
						'By default, everything in the GitButler client is free to use. You can opt in to crediting us as the committer in your virtual branch commits to help spread the word. <a href="https://github.com/gitbutlerapp/gitbutler-docs/blob/d81a23779302c55f8b20c75bf7842082815b4702/content/docs/features/virtual-branches/committer-mark.mdx">Learn more</a>'
				},
				autoFetch: {
					title: 'Auto-fetch frequency',
					oneMinute: '1 minute',
					fiveMinutes: '5 minutes',
					tenMinutes: '10 minutes',
					fifteenMinutes: '15 minutes',
					none: 'None'
				}
			},
			integrations: {
				label: 'Integrations',
				autoFillPr: {
					title: 'Auto-fill PR/MR descriptions from commit',
					caption:
						"When creating a pull request or merge request for a branch with just one commit, automatically use that commit's message as the PR/MR title and description."
				},
				github: {
					authenticated: 'GitHub authenticated',
					authFailed: 'GitHub authentication failed',
					invalidToken: 'Invalid token or network error',
					invalidTokenOrHost: 'Invalid token or host',
					loadFailed: 'Failed to load GitHub accounts',
					tryAgain: 'Try again',
					caption: 'Allows you to create Pull Requests',
					copyCode: 'Copy the following verification code:',
					copyToClipboard: 'Copy to Clipboard',
					navigateToGitHub: 'Navigate to the GitHub activation page and paste the code you copied.',
					openGitHub: 'Open GitHub activation page',
					checkStatus: 'Check the status',
					addPat: 'Add Personal Access Token',
					cancel: 'Cancel',
					addAccount: 'Add account',
					addAnotherAccount: 'Add another account',
					addGhe: 'Add GitHub Enterprise Account',
					gheCaption:
						'To connect to your GitHub Enterprise API, allow-list it in the app’s CSP settings.<br />See <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">docs for details</a>',
					apiBaseUrl: 'API Base URL',
					apiBaseUrlHelper:
						"This should be the root URL of the API. For example, if your GitHub Enterprise Server's hostname is github.acme-inc.com, then set the base URL to https://github.acme-inc.com/api/v3",
					personalAccessToken: 'Personal Access Token',
					credentialsPersisted:
						'🔒 Credentials are persisted locally in your OS Keychain / Credential Manager.',
					authorizeAccount: 'Authorize GitHub Account'
				}
			},
			ai: {
				label: 'AI Options',
				about:
					'GitButler supports multiple AI providers: OpenAI and Anthropic (via API or your own key), plus local models through Ollama and LM Studio.',
				useButlerApi: 'Use GitButler API',
				bringYourOwn: 'Your own key',
				openAi: {
					title: 'Open AI',
					keyPrompt: 'Do you want to provide your own key?',
					signInMessage: 'Please sign in to use the GitButler API.',
					butlerApiNote: 'GitButler uses OpenAI API for commit messages and branch names.',
					keyLabel: 'API key',
					modelVersion: 'Model version',
					customEndpoint: 'Custom endpoint'
				},
				anthropic: {
					title: 'Anthropic',
					keyPrompt: 'Do you want to provide your own key?',
					signInMessage: 'Please sign in to use the GitButler API.',
					butlerApiNote: 'GitButler uses Anthropic API for commit messages and branch names.',
					keyLabel: 'API key',
					modelVersion: 'Model version'
				},
				ollama: {
					title: 'Ollama 🦙',
					configTitle: 'Configuring Ollama',
					configContent:
						'To connect to your Ollama endpoint, <b>allow-list it in the app\'s CSP settings</b>.<br />See the <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">docs for details</a>'
				},
				lmStudio: {
					title: 'LM Studio',
					endpoint: 'Endpoint',
					model: 'Model',
					configTitle: 'Configuring LM Studio',
					configContent:
						'<p>Connecting to your LM Studio endpoint requires that you do two things:</p><p>1. <span class="text-bold">Allow-list it in the CSP settings for the application</span>. You can find more details on how to do that in the <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">GitButler docs</a>.</p><p>2. <span class="text-bold">Enable CORS support in LM Studio</span>. You can find more details on how to do that in the <a href="https://lmstudio.ai/docs/cli/server-start#enable-cors-support">LM Studio docs</a>.</p>'
				},
				contextLength: {
					title: 'Amount of provided context',
					caption: 'How many characters of your git diff should be provided to AI'
				},
				customPrompts: {
					title: 'Custom AI prompts',
					description:
						"GitButler's AI assistant generates commit messages and branch names. Use default prompts or create your own. Assign prompts in the project settings."
				},
				modelNames: {
					gpt5: 'GPT 5',
					gpt5Mini: 'GPT 5 Mini',
					o3Mini: 'o3 Mini',
					o1Mini: 'o1 Mini',
					gpt4oMini: 'GPT 4o mini',
					gpt41: 'GPT 4.1',
					gpt41Mini: 'GPT 4.1 mini (recommended)',
					haiku: 'Haiku',
					sonnet35: 'Sonnet 3.5',
					sonnet37: 'Sonnet 3.7 (recommended)',
					sonnet4: 'Sonnet 4',
					opus4: 'Opus 4'
				}
			},
			telemetry: {
				label: 'Telemetry',
				description:
					'GitButler uses telemetry strictly to help us improve the client. We do not collect any personal information, unless explicitly allowed below. <a href="https://gitbutler.com/privacy">Privacy policy</a>',
				request:
					'We kindly ask you to consider keeping these settings enabled as it helps us catch issues more quickly. If you choose to disable them, please feel free to share your feedback on our <a href="https://discord.gg/MmFkmaJ42D">Discord</a>.',
				errorReporting: {
					title: 'Error reporting',
					caption: 'Toggle reporting of application crashes and errors.'
				},
				usageMetrics: {
					title: 'Usage metrics',
					caption: 'Toggle sharing of usage statistics.'
				},
				nonAnonMetrics: {
					title: 'Non-anonymous usage metrics',
					caption: 'Toggle sharing of identifiable usage statistics.'
				}
			},
			experimental: {
				label: 'Experimental',
				about:
					'Flags for features in development or beta. Features may not work fully.<br />Use at your own risk.',
				apply3: {
					title: 'New apply to workspace',
					caption: 'Use the V3 version of apply and unapply operations for workspace changes.'
				},
				fMode: {
					title: 'F Mode Navigation',
					caption:
						'Enable F mode for quick keyboard navigation to buttons using two-letter shortcuts.'
				},
				newRebase: {
					title: 'New rebase engine',
					caption: 'Use the new graph-based rebase engine for stack operations.'
				},
				singleBranch: {
					title: 'Single-branch mode',
					caption: 'Stay in the workspace view when leaving the gitbutler/workspace branch.'
				},
				irc: {
					title: 'IRC',
					caption: 'Enable experimental in-app chat.',
					serverLabel: 'Server'
				}
			},
			organizations: {
				label: 'Organizations',
				createButton: 'Create an Organizaton'
			},
			footer: {
				social: {
					docs: 'Docs',
					discord: 'Our Discord'
				}
			}
		},
		project: {
			title: 'Project settings',
			project: {
				label: 'Project'
			},
			git: {
				label: 'Git stuff',
				allowForcePush: {
					title: 'Allow force pushing',
					caption:
						'Force pushing allows GitButler to override branches even if they were pushed to remote. GitButler will never force push to the target branch.'
				},
				forcePushProtection: {
					title: 'Force push protection',
					caption:
						"Protect remote commits during force pushes. This will use Git's safer force push flags to avoid overwriting remote commit history."
				}
			},
			ai: {
				label: 'AI options',
				description:
					"GitButler supports the use of OpenAI and Anthropic to provide commit message and branch name generation. This works either through GitButler's API or in a bring your own key configuration and can be configured in the main preferences screen.",
				enableGeneration: {
					title: 'Enable branch and commit message generation',
					caption:
						'If enabled, diffs will be sent to OpenAI or Anthropic\'s servers when pressing the "Generate message" and "Generate branch name" button.'
				},
				enableExperimental: {
					title: 'Enable experimental AI features',
					caption:
						'If enabled, you will be able to access the AI features currently in development. This also requires you to use OpenAI through GitButler in order for the features to work.'
				},
				customPrompts: {
					title: 'Custom prompts',
					description:
						'You can apply your own custom prompts to the project. By default, the project uses GitButler prompts, but you can create your own prompts in the general settings.',
					button: 'Customize prompts'
				}
			},
			agent: {
				label: 'Agent',
				guideText:
					'Get the full guide to Agents in GitButler in <a href="https://docs.gitbutler.com/features/agents-tab#installing-claude-code">our documentation</a>',
				autoCommit: {
					title: 'Auto-commit after completion',
					caption:
						'Automatically commit and rename branches when Claude Code finishes. Disable to review manually before committing.'
				},
				useConfiguredModel: {
					title: 'Use configured model',
					caption: 'Use the model configured in .claude/settings.json.'
				},
				newlineOnEnter: {
					title: 'Newline on Enter',
					caption: 'Use Enter for line breaks and Cmd+Enter to submit.'
				},
				notifyOnCompletion: 'Notify when finishes',
				notifyOnPermissionRequest: 'Notify when needs permission',
				dangerousPermissions: {
					title: '⚠ Dangerously allow all permissions',
					caption:
						'Skips all permission prompts and allows Claude Code unrestricted access. Use with extreme caution.'
				}
			},
			experimental: {
				label: 'Experimental',
				ignoreCertificate: {
					title: 'Ignore host certificate checks',
					caption: 'Enabling this will ignore host certificate checks when authenticating with ssh.'
				}
			},
			details: {
				projectPath: 'Project path',
				projectName: 'Project name',
				projectNamePlaceholder: "Project name can't be empty",
				projectDescription: 'Project description',
				runGitHooks: {
					title: 'Run Git hooks',
					caption:
						'Enabling this will run git pre-push, pre and post commit, and commit-msg hooks you have configured in your repository.'
				}
			},
			disableCodegen: {
				title: 'Disable codegen',
				caption: 'Hides the codegen button in the branch headers.'
			},
			baseBranch: {
				loading: 'Loading remote branches...',
				title: 'Remote configuration',
				caption:
					"Lets you choose where to push code and set the target branch for contributions. The target branch is usually the \"production\" branch like 'origin/master' or 'upstream/main.' This section helps ensure your code goes to the correct remote and branch for integration.",
				currentTargetBranch: 'Current target branch',
				createBranchesOnRemote: 'Create branches on remote',
				activeBranchesWarning:
					'You have {count} active branch in your workspace. Please clear the workspace before switching the base branch. | You have {count} active branches in your workspace. Please clear the workspace before switching the base branch.',
				switchingBranches: 'Switching branches...',
				updateConfiguration: 'Update configuration',
				errorListingBranches: 'We got an error trying to list your remote branches'
			},
			remove: {
				title: 'Remove project',
				caption: 'Removing projects only clears configuration — your code stays safe.',
				success: 'Project deleted',
				error: 'Failed to delete project'
			}
		},
		error: {
			notFound: 'Settings page {id} not Found.'
		}
	}
};

export default locale;
