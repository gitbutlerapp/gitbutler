import type { DefineLocaleMessage } from '$lib/i18n/i18nLocale';

const locale: DefineLocaleMessage = {
	welcome: {
		auth: {
			title: '登录或注册',
			description: '登录以获取智能自动化特性，包括智能的分支创建和提交信息生成。',
			loginButton: '登录 / 注册',
			cancel: '取消',
			copyLoginLink: '复制登录链接'
		}
	},
	settings: {
		general: {
			title: '全局设置',
			general: {
				label: '常规',
				signingOut: {
					title: '退出登录',
					caption: '需要休息一下吗？点击这里退出登录，然后放松去吧。',
					button: '退出登录'
				},
				language: {
					title: '语言',
					changeSuccess: '语言更改成功'
				},
				codeEditor: {
					title: '默认代码编辑器'
				},
				autoUpdate: {
					title: '自动检查更新',
					caption: '自动检查更新。当你需要时你仍然可以手动进行检查。'
				},
				cliInstall: {
					title: '安装 GitButler CLI <code class="code-string">but</code>',
					captionPackageManager:
						'<code>but</code> CLI 由你的包管理器管理。请使用包管理器来安装、更新或删除它。',
					captionWindows:
						'在 Windows 上，你可以手动将可执行文件 (<code>`but`</code>) 复制到 PATH 中的目录。点击“显示命令”查看说明。',
					captionUnix:
						'将 GitButler CLI (<code>`but`</code>) 安装到你的 PATH 中，这样你就可以在终端中使用它。此操作需要管理员权限，或者你也可以自己手动创建符号链接。',
					installButton: '安装 But CLI',
					showCommandButton: '显示命令'
				},
				removeProjects: {
					title: '移除所有项目',
					caption:
						'你可以从 GitButler 应用中删除所有项目。<br />你的代码仍然安全，它只会清除应用的配置。',
					button: '移除项目……',
					modalTitle: '移除所有项目',
					modalMessage: '你确定要移除所有 GitButler 项目吗？',
					removeButton: '移除',
					cancelButton: '取消',
					success: '所有配置数据已删除',
					errorFailedDelete: '删除项目失败'
				},
				profileUpdate: {
					fullName: '全名',
					email: '邮箱',
					updateButton: '更新个人资料',
					success: '个人资料已更新',
					errorFailedUpdate: '更新用户失败',
					errorInvalidFile: '请使用有效的图片文件'
				}
			},
			appearance: {
				label: '外观',
				theme: {
					title: '主题',
					light: '浅色',
					dark: '深色',
					system: '跟随系统'
				},
				fileListMode: {
					title: '默认文件列表模式',
					caption: '设置默认的文件列表视图（也可以在各个位置单独更改）。',
					listView: '列表视图',
					treeView: '树状视图'
				},
				filePathFirst: {
					title: '文件路径优先',
					caption: '在文件列表中，完整的文件路径会显示在文件名的前面。'
				},
				diffPreview: {
					title: '差异预览'
				},
				diffFont: {
					title: '字体',
					caption: '设置差异视图的字体。第一个字体名称是默认字体，其他的是备用字体。'
				},
				diffLigatures: {
					title: '允许字体连字'
				},
				tabSize: {
					title: 'Tab 大小',
					caption: '差异视图中每个 Tab 的空格数。'
				},
				softWrap: {
					title: '自动换行',
					caption: '在差异视图中自动换行长行以适应可视区域。'
				},
				linesContrast: {
					title: '行对比度',
					caption: '差异视图中添加、删除和相关上下文行的对比度。',
					light: '浅色',
					medium: '中等',
					strong: '强烈'
				},
				colorBlindFriendly: {
					title: '色盲友好的色彩',
					caption: '使用蓝色和橙色代替绿色和红色，以提高色觉缺陷用户的可访问性。'
				},
				inlineWordDiffs: {
					title: '内联显示单词级别的差异',
					caption:
						'此功能不显示单独的删除和添加行，而是将它们显示为单行，并突出显示添加和删除的单词。'
				},
				scrollbarOnScroll: {
					title: '滚动时显示滚动条',
					caption: '仅在滚动时显示滚动条。'
				},
				scrollbarOnHover: {
					title: '悬停时显示滚动条',
					caption: '仅在将鼠标悬停在可滚动区域上时显示滚动条。'
				},
				scrollbarAlways: {
					title: '始终显示滚动条'
				},
				stagingBehavior: {
					stageAll: {
						title: '暂存所有文件',
						caption: '提交时暂存分配给堆栈的所有文件。如果没有文件被暂存，将暂存所有未分配的文件。'
					},
					stageSelection: {
						title: '暂存选定的文件',
						caption:
							'提交时将被选择的已分配文件暂存到堆栈。如果没有选定文件，则暂存所有文件。<br />如果没有已分配文件，则暂存所有被选择的未分配文件，如果这时没有文件被选择，则会暂存所有未分配文件。'
					},
					stageNone: {
						title: '不自动暂存文件',
						caption: '不自动暂存任何文件。<br />你更喜欢自己动手的开发方式。'
					}
				}
			},
			lanesAndBranches: {
				label: '泳道和分支',
				newLanesPlacement: {
					title: '在左侧放置新泳道',
					caption: '默认情况下，新泳道会被添加到最右侧的位置。启用此选项可以将它们换到最左侧。'
				},
				autoSelectCreation: {
					title: '创建分支时自动选择文本',
					caption: '在创建新分支时自动选择分支名称输入框中的预填充文本，方便输入你自己的名称。'
				},
				autoSelectRename: {
					title: '重命名分支时自动选择文本',
					caption: '重命名分支或泳道时自动选择文本，方便替换整个名称。'
				}
			},
			git: {
				label: 'Git 相关',
				committerCredit: {
					title: '将 GitButler 标记为提交者',
					caption:
						'默认情况下，GitButler 客户端中的所有功能都可以免费使用。你可以选择将我们标记为你的虚拟分支提交中的提交者，从而帮助宣传我们。<a href="https://github.com/gitbutlerapp/gitbutler-docs/blob/d81a23779302c55f8b20c75bf7842082815b4702/content/docs/features/virtual-branches/committer-mark.mdx">了解更多</a>'
				},
				autoFetch: {
					title: '自动 fetch 的频率',
					oneMinute: '1 分钟',
					fiveMinutes: '5 分钟',
					tenMinutes: '10 分钟',
					fifteenMinutes: '15 分钟',
					none: '无'
				}
			},
			integrations: {
				label: '集成',
				autoFillPr: {
					title: '从提交自动填充 PR/MR 描述',
					caption:
						'为只有一个提交的分支创建 Pull Request 或 Merge Request 时，将自动使用该提交的信息作为 PR/MR 的标题和描述。'
				},
				github: {
					authenticated: 'GitHub 已认证',
					authFailed: 'GitHub 认证失败',
					invalidToken: '无效的 token 或网络错误',
					invalidTokenOrHost: '无效的 token 或主机',
					loadFailed: '加载 GitHub 账户失败',
					tryAgain: '重试',
					caption: '允许你创建 Pull Request',
					copyCode: '复制以下验证码：',
					copyToClipboard: '复制到剪贴板',
					navigateToGitHub: '导航到 GitHub 激活页面并粘贴你复制的验证码。',
					openGitHub: '打开 GitHub 激活页面',
					checkStatus: '检查状态',
					addPat: '添加个人访问令牌（PAT）',
					cancel: '取消',
					addAccount: '添加账户',
					addAnotherAccount: '添加另一个账户',
					addGhe: '添加 GitHub Enterprise 账户',
					gheCaption:
						'要连接到你的 GitHub Enterprise API，需要在应用程序的 CSP 设置中将其添加到允许列表。<br />查看<a href="https://docs.gitbutler.com/troubleshooting/custom-csp">文档以了解详情</a>',
					apiBaseUrl: 'API 基础 URL',
					apiBaseUrlHelper:
						'这应该是 API 的根 URL。例如，如果你的 GitHub Enterprise Server 的主机名是 github.acme-inc.com，则将基础 URL 设置为 https://github.acme-inc.com/api/v3',
					personalAccessToken: '个人访问令牌（PAT）',
					credentialsPersisted: '🔒 凭据存储在你的操作系统 Keychain / Credential Manager 中。',
					authorizeAccount: '授权 GitHub 账户'
				}
			},
			ai: {
				label: 'AI 选项',
				about:
					'GitButler 支持多个 AI 提供商：OpenAI 和 Anthropic（通过 API 或者你自己的密钥），以及通过 Ollama 和 LM Studio 支持的本地模型。',
				useButlerApi: '使用 GitButler API',
				bringYourOwn: '使用你自己的密钥',
				openAi: {
					title: 'Open AI',
					keyPrompt: '想提供你自己的密钥吗？',
					signInMessage: '请登录以使用 GitButler API。',
					butlerApiNote: 'GitButler 使用 OpenAI API 生成提交信息和分支名称。',
					keyLabel: 'API 密钥',
					modelVersion: '模型版本',
					customEndpoint: '自定义端点'
				},
				anthropic: {
					title: 'Anthropic',
					keyPrompt: '想提供你自己的密钥吗？',
					signInMessage: '请登录以使用 GitButler API。',
					butlerApiNote: 'GitButler 使用 Anthropic API 生成提交信息和分支名称。',
					keyLabel: 'API 密钥',
					modelVersion: '模型版本'
				},
				ollama: {
					title: 'Ollama 🦙',
					configTitle: '配置 Ollama',
					configContent:
						'要连接到你的 Ollama 端点，<b>需要在应用的 CSP 设置中将其添加到允许列表</b>。<br />查看<a href="https://docs.gitbutler.com/troubleshooting/custom-csp">文档以了解详情</a>'
				},
				lmStudio: {
					title: 'LM Studio',
					endpoint: '端点',
					model: '模型',
					configTitle: '配置 LM Studio',
					configContent:
						'<p>连接到你的 LM Studio 端点需要做两件事：</p><p>1. <span class="text-bold">在应用程序的 CSP 设置中将其添加到允许列表</span>。你可以在 <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">GitButler 文档</a>中找到更多详细信息。</p><p>2. <span class="text-bold">在 LM Studio 中启用 CORS 支持</span>。你可以在 <a href="https://lmstudio.ai/docs/cli/server-start#enable-cors-support">LM Studio 文档</a>中找到更多详细信息。</p>'
				},
				contextLength: {
					title: '提供的上下文量',
					caption: '应该向 AI 提供多少字符的 git diff 结果'
				},
				customPrompts: {
					title: '自定义 AI 提示词',
					description:
						'GitButler 的 AI 助手会生成提交信息和分支名称。你可以使用默认的提示词或创建你自己的，并在项目设置中分配它们。'
				},
				modelNames: {
					gpt5: 'GPT 5',
					gpt5Mini: 'GPT 5 Mini',
					o3Mini: 'o3 Mini',
					o1Mini: 'o1 Mini',
					gpt4oMini: 'GPT 4o mini',
					gpt41: 'GPT 4.1',
					gpt41Mini: 'GPT 4.1 mini（推荐）',
					haiku: 'Haiku',
					sonnet35: 'Sonnet 3.5',
					sonnet37: 'Sonnet 3.7（推荐）',
					sonnet4: 'Sonnet 4',
					opus4: 'Opus 4'
				}
			},
			telemetry: {
				label: '遥测',
				description:
					'GitButler 使用遥测将严格用于帮助我们改进客户端。我们不会收集任何个人信息，除非你在以下选项中明确允许。<a href="https://gitbutler.com/privacy">隐私政策</a>',
				request:
					'我们恳请你考虑保持这些设置启用，因为这有助于我们更快地捕获问题。如果你选择了禁用它们，请随时在我们的 <a href="https://discord.gg/MmFkmaJ42D">Discord</a> 上分享你的反馈。',
				errorReporting: {
					title: '错误报告',
					caption: '是否发送应用程序崩溃和出现错误时的报告。'
				},
				usageMetrics: {
					title: '使用情况指标',
					caption: '是否共享使用情况统计数据。'
				},
				nonAnonMetrics: {
					title: '非匿名的使用情况指标',
					caption: '是否共享可供被识别的使用情况统计数据。'
				}
			},
			experimental: {
				label: '实验性功能',
				about:
					'开发中或测试版功能标志的开关。这些功能可能无法完全正常工作。<br />使用它们的风险自负。',
				apply3: {
					title: '新的应用到工作区',
					caption: '使用 V3 版本的应用和取消应用操作进行工作区更改。'
				},
				fMode: {
					title: 'F 模式导航',
					caption: '启用 F 模式，使用两个字母的快捷键快速导航到按钮。'
				},
				newRebase: {
					title: '新的变基引擎',
					caption: '使用新的基于图的变基引擎进行堆栈操作。'
				},
				singleBranch: {
					title: '单分支模式',
					caption: '离开 gitbutler/workspace 分支时保持在工作区视图中。'
				},
				irc: {
					title: 'IRC',
					caption: '启用实验性的应用内聊天。',
					serverLabel: '服务器'
				}
			},
			organizations: {
				label: '组织',
				createButton: '创建组织'
			},
			footer: {
				social: {
					docs: '文档',
					discord: '我们的 Discord'
				}
			}
		},
		project: {
			title: '项目设置',
			project: {
				label: '项目'
			},
			git: {
				label: 'Git 相关',
				allowForcePush: {
					title: '允许强制推送',
					caption:
						'强制推送允许 GitButler 覆盖分支，即使它们已经被推送到远程。GitButler 永远不会强制推送目标分支。'
				},
				forcePushProtection: {
					title: '强制推送保护',
					caption:
						'在强制推送期间保护远程的提交。这将使用 Git 更安全的强制推送标记来避免覆盖远程的提交历史。'
				}
			},
			ai: {
				label: 'AI 选项',
				description:
					'GitButler 支持使用 OpenAI 和 Anthropic 来生成提交信息和分支名称。这可以通过 GitButler 的 API 或使用你自己的密钥来实现，它们可以在全局设置中进行配置。',
				enableGeneration: {
					title: '启用分支和提交信息生成',
					caption:
						'如果启用，当按下“生成提交信息”和“生成分支名称”按钮时，差异将被发送到 OpenAI 或 Anthropic 的服务器。'
				},
				enableExperimental: {
					title: '启用实验性 AI 功能',
					caption:
						'如果启用，你将能够访问当前在开发中的 AI 功能。这还需要你通过 GitButler 使用 OpenAI，以便功能正常工作。'
				},
				customPrompts: {
					title: '自定义提示词',
					description:
						'你可以将自己的自定义提示词应用到项目。默认情况下，项目使用 GitButler 的提示词，不过你也可以在全局设置中创建你自己的。',
					button: '自定义提示词'
				}
			},
			agent: {
				label: '智能代理',
				guideText:
					'在<a href="https://docs.gitbutler.com/features/agents-tab#installing-claude-code">我们的文档</a>中获取 GitButler 智能代理的完整指南',
				autoCommit: {
					title: '完成后自动提交',
					caption: '当 Claude Code 完成时自动提交并重命名分支。禁用以在提交前手动进行审查。'
				},
				useConfiguredModel: {
					title: '使用已配置的模型',
					caption: '使用 .claude/settings.json 中配置的模型。'
				},
				newlineOnEnter: {
					title: 'Enter 键换行',
					caption: '使用 Enter 键换行，Cmd+Enter 提交。'
				},
				notifyOnCompletion: '完成时发送通知',
				notifyOnPermissionRequest: '需要授权时发送通知',
				dangerousPermissions: {
					title: '⚠ 危险：允许所有权限',
					caption: '跳过所有的权限提示，允许 Claude Code 无限制访问。请极度谨慎地使用。'
				}
			},
			experimental: {
				label: '实验性功能',
				ignoreCertificate: {
					title: '忽略主机证书检查',
					caption: '启用此选项将在使用 ssh 进行认证时忽略对主机证书的检查。'
				}
			},
			details: {
				projectPath: '项目路径',
				projectName: '项目名称',
				projectNamePlaceholder: '项目名称不能为空',
				projectDescription: '项目描述',
				runGitHooks: {
					title: '运行 Git hooks',
					caption:
						'启用此选项将运行你在仓库中配置的 git pre-push、pre / post commit 和 commit-msg hooks。'
				}
			},
			disableCodegen: {
				title: '禁用代码生成',
				caption: '隐藏分支顶部的代码生成按钮。'
			},
			baseBranch: {
				loading: '正在加载远程分支……',
				title: '远程配置',
				caption:
					"允许你选择要将代码推送到哪里，以及设置要做贡献的目标分支。目标分支通常是“生产”分支，如 'origin/master' 或 'upstream/main'。此部分设置可以帮助你确保将代码推送到正确的远程和分支以便用于集成。",
				currentTargetBranch: '当前目标分支',
				createBranchesOnRemote: '在远程创建分支',
				activeBranchesWarning:
					'你的工作区中有 {count} 个活跃分支。请在切换基本分支之前清理工作区。',
				switchingBranches: '正在切换分支……',
				updateConfiguration: '更新配置',
				errorListingBranches: '在尝试列出远程分支时出现了错误'
			},
			remove: {
				title: '移除项目',
				caption: '移除项目只会清除配置，你的代码保持安全。',
				success: '项目已删除',
				error: '删除项目失败'
			}
		},
		error: {
			notFound: '未找到设置页面 {id}。'
		}
	}
};

export default locale;
