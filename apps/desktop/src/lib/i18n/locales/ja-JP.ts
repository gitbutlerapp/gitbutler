import type { DefineLocaleMessage } from '$lib/i18n/i18nLocale';

const locale: DefineLocaleMessage = {
	welcome: {
		auth: {
			title: 'ログインまたはサインアップ',
			description:
				'ログインして、インテリジェントなブランチ作成やコミットメッセージ生成などのスマート自動化機能にアクセスしましょう。',
			loginButton: 'ログイン / サインアップ',
			cancel: 'キャンセル',
			copyLoginLink: 'ログインリンクをコピー'
		}
	},
	settings: {
		general: {
			title: 'グローバル設定',
			general: {
				label: '一般',
				signingOut: {
					title: 'サインアウト',
					caption: 'ちょっと休憩しませんか？ここをクリックしてログアウトしてリラックスしましょう。',
					button: 'ログアウト'
				},
				language: {
					title: '言語',
					changeSuccess: '言語を変更しました'
				},
				codeEditor: {
					title: 'デフォルトのコードエディター'
				},
				autoUpdate: {
					title: '自動的にアップデートを確認',
					caption: '自動的にアップデートを確認します。必要に応じて手動でも確認できます。'
				},
				cliInstall: {
					title: 'GitButler CLI <code class="code-string">but</code> をインストール',
					captionPackageManager:
						'<code>but</code> CLI はパッケージマネージャーによって管理されています。インストール、更新、削除にはパッケージマネージャーを使用してください。',
					captionWindows:
						'Windows では、実行ファイル (<code>`but`</code>) を PATH 内のディレクトリに手動でコピーできます。手順は「コマンドを表示」をクリックしてください。',
					captionUnix:
						'GitButler CLI (<code>`but`</code>) を PATH にインストールして、ターミナルから使用できるようにします。この操作には管理者権限が必要です。または、手動でシンボリックリンクを作成することもできます。',
					installButton: 'But CLI をインストール',
					showCommandButton: 'コマンドを表示'
				},
				removeProjects: {
					title: 'すべてのプロジェクトを削除',
					caption:
						'GitButler アプリからすべてのプロジェクトを削除できます。<br />コードは安全に保たれます。設定がクリアされるだけです。',
					button: 'プロジェクトを削除…',
					modalTitle: 'すべてのプロジェクトを削除',
					modalMessage: '本当にすべての GitButler プロジェクトを削除しますか？',
					removeButton: '削除',
					cancelButton: 'キャンセル',
					success: 'すべての設定データを削除しました',
					errorFailedDelete: 'プロジェクトの削除に失敗しました'
				},
				profileUpdate: {
					fullName: '氏名',
					email: 'メール',
					updateButton: 'プロフィールを更新',
					success: 'プロフィールを更新しました',
					errorFailedUpdate: 'ユーザーの更新に失敗しました',
					errorInvalidFile: '有効な画像ファイルを使用してください'
				}
			},
			appearance: {
				label: '外観',
				theme: {
					title: 'テーマ',
					light: 'ライト',
					dark: 'ダーク',
					system: 'システム設定に従う'
				},
				fileListMode: {
					title: 'デフォルトのファイルリストモード',
					caption: 'デフォルトのファイルリスト表示を設定します（場所ごとに変更可能）。',
					listView: 'リスト表示',
					treeView: 'ツリー表示'
				},
				filePathFirst: {
					title: 'ファイルパスを先に表示',
					caption: 'ファイルリストでファイル名の前に完全なファイルパスを表示します。'
				},
				diffPreview: {
					title: '差分プレビュー'
				},
				diffFont: {
					title: 'フォント',
					caption:
						'差分ビューのフォントを設定します。最初のフォント名がデフォルト、その他はフォールバックです。'
				},
				diffLigatures: {
					title: 'フォントの合字を許可'
				},
				tabSize: {
					title: 'タブサイズ',
					caption: '差分ビューでのタブあたりのスペース数。'
				},
				softWrap: {
					title: 'ソフトラップ',
					caption: '差分ビューで長い行をビューポートに収まるようにソフトラップします。'
				},
				linesContrast: {
					title: '行のコントラスト',
					caption: '差分の追加行、削除行、コンテキスト行のコントラスト。',
					light: '弱い',
					medium: '中程度',
					strong: '強い'
				},
				colorBlindFriendly: {
					title: '色覚特性に配慮した色',
					caption:
						'緑と赤の代わりに青とオレンジを使用して、<br />色覚特性を持つ方のアクセシビリティを向上させます。'
				},
				inlineWordDiffs: {
					title: '単語差分をインライン表示',
					caption:
						'削除と追加の行を別々に表示する代わりに、この機能は追加された単語と削除された単語をハイライトした単一の行を表示します。'
				},
				scrollbarOnScroll: {
					title: 'スクロール時にスクロールバー表示',
					caption: 'スクロールしているときのみスクロールバーを表示します。'
				},
				scrollbarOnHover: {
					title: 'ホバー時にスクロールバー表示',
					caption: 'スクロール可能なエリアにマウスをホバーしたときのみスクロールバーを表示します。'
				},
				scrollbarAlways: {
					title: '常にスクロールバーを表示'
				},
				stagingBehavior: {
					stageAll: {
						title: 'すべてのファイルをステージ',
						caption:
							'コミット時にスタックに割り当てられたすべてのファイルをステージします。ステージされたファイルがない場合は、すべての未割り当てファイルをステージします。'
					},
					stageSelection: {
						title: '選択したファイルをステージ',
						caption:
							'コミット時に選択された割り当てファイルをスタックにステージします。ファイルが選択されていない場合は、すべてのファイルをステージします。割り当てられたファイルがない場合は、すべての選択された未割り当てファイルをステージします。<br />ファイルが選択されていない場合は、すべての未割り当てファイルをステージします。'
					},
					stageNone: {
						title: 'ファイルを自動的にステージしない',
						caption: 'ファイルを自動的にステージしません。<br />自分でやりたい派の開発者向けです。'
					}
				}
			},
			lanesAndBranches: {
				label: 'レーンとブランチ',
				newLanesPlacement: {
					title: '新しいレーンを左側に配置',
					caption:
						'デフォルトでは、新しいレーンは最も右の位置に追加されます。これを有効にすると、最も左の位置に追加されます。'
				},
				autoSelectCreation: {
					title: 'ブランチ作成時にテキストを自動選択',
					caption:
						'新しいブランチを作成するときに、ブランチ名フィールドの事前入力されたテキストを自動的に選択し、独自の名前を入力しやすくします。'
				},
				autoSelectRename: {
					title: 'ブランチ名変更時にテキストを自動選択',
					caption:
						'ブランチまたはレーンの名前を変更するときにテキストを自動的に選択し、名前全体を置き換えやすくします。'
				}
			},
			git: {
				label: 'Git 関連',
				committerCredit: {
					title: 'GitButler をコミッターとしてクレジット',
					caption:
						'デフォルトでは、GitButler クライアントのすべてが無料で使用できます。仮想ブランチのコミットで私たちをコミッターとしてクレジットすることで、宣伝に協力できます。<a href="https://github.com/gitbutlerapp/gitbutler-docs/blob/d81a23779302c55f8b20c75bf7842082815b4702/content/docs/features/virtual-branches/committer-mark.mdx">詳細を見る</a>'
				},
				autoFetch: {
					title: '自動フェッチ頻度',
					oneMinute: '1分',
					fiveMinutes: '5分',
					tenMinutes: '10分',
					fifteenMinutes: '15分',
					none: 'なし'
				}
			},
			integrations: {
				label: '統合',
				autoFillPr: {
					title: 'コミットから PR/MR の説明を自動入力',
					caption:
						'コミットが1つだけのブランチのプルリクエストまたはマージリクエストを作成するときに、そのコミットのメッセージを PR/MR のタイトルと説明として自動的に使用します。'
				},
				github: {
					authenticated: 'GitHub 認証済み',
					authFailed: 'GitHub 認証に失敗しました',
					invalidToken: '無効なトークンまたはネットワークエラー',
					invalidTokenOrHost: '無効なトークンまたはホスト',
					loadFailed: 'GitHub アカウントの読み込みに失敗しました',
					tryAgain: '再試行',
					caption: 'プルリクエストを作成できます',
					copyCode: '以下の確認コードをコピーしてください：',
					copyToClipboard: 'クリップボードにコピー',
					navigateToGitHub:
						'GitHub アクティベーションページに移動して、コピーしたコードを貼り付けてください。',
					openGitHub: 'GitHub アクティベーションページを開く',
					checkStatus: 'ステータスを確認',
					addPat: 'Personal Access Token を追加',
					cancel: 'キャンセル',
					addAccount: 'アカウントを追加',
					addAnotherAccount: '別のアカウントを追加',
					addGhe: 'GitHub Enterprise アカウントを追加',
					gheCaption:
						'GitHub Enterprise API に接続するには、アプリの CSP 設定で許可リストに追加する必要があります。<br />詳細は<a href="https://docs.gitbutler.com/troubleshooting/custom-csp">ドキュメント</a>を参照してください',
					apiBaseUrl: 'API ベース URL',
					apiBaseUrlHelper:
						'これは API のルート URL である必要があります。たとえば、GitHub Enterprise Server のホスト名が github.acme-inc.com の場合、ベース URL を https://github.acme-inc.com/api/v3 に設定します',
					personalAccessToken: 'Personal Access Token',
					credentialsPersisted:
						'🔒 認証情報は OS の Keychain / Credential Manager にローカルに保存されます。',
					authorizeAccount: 'GitHub アカウントを認証'
				}
			},
			ai: {
				label: 'AI オプション',
				about:
					'GitButler は複数の AI プロバイダーをサポートしています：OpenAI と Anthropic（API または独自のキー経由）、さらに Ollama と LM Studio を介したローカルモデル。',
				useButlerApi: 'GitButler API を使用',
				bringYourOwn: '独自のキーを使用',
				openAi: {
					title: 'Open AI',
					keyPrompt: '独自のキーを提供しますか？',
					signInMessage: 'GitButler API を使用するにはサインインしてください。',
					butlerApiNote:
						'GitButler は OpenAI API を使用してコミットメッセージとブランチ名を生成します。',
					keyLabel: 'API キー',
					modelVersion: 'モデルバージョン',
					customEndpoint: 'カスタムエンドポイント'
				},
				anthropic: {
					title: 'Anthropic',
					keyPrompt: '独自のキーを提供しますか？',
					signInMessage: 'GitButler API を使用するにはサインインしてください。',
					butlerApiNote:
						'GitButler は Anthropic API を使用してコミットメッセージとブランチ名を生成します。',
					keyLabel: 'API キー',
					modelVersion: 'モデルバージョン'
				},
				ollama: {
					title: 'Ollama 🦙',
					configTitle: 'Ollama の設定',
					configContent:
						'Ollama エンドポイントに接続するには、<b>アプリの CSP 設定で許可リストに追加</b>する必要があります。<br />詳細は<a href="https://docs.gitbutler.com/troubleshooting/custom-csp">ドキュメント</a>を参照してください'
				},
				lmStudio: {
					title: 'LM Studio',
					endpoint: 'エンドポイント',
					model: 'モデル',
					configTitle: 'LM Studio の設定',
					configContent:
						'<p>LM Studio エンドポイントに接続するには、次の2つの操作が必要です：</p><p>1. <span class="text-bold">アプリケーションの CSP 設定で許可リストに追加する</span>。詳細は <a href="https://docs.gitbutler.com/troubleshooting/custom-csp">GitButler ドキュメント</a>を参照してください。</p><p>2. <span class="text-bold">LM Studio で CORS サポートを有効にする</span>。詳細は <a href="https://lmstudio.ai/docs/cli/server-start#enable-cors-support">LM Studio ドキュメント</a>を参照してください。</p>'
				},
				contextLength: {
					title: '提供されるコンテキストの量',
					caption: 'AI に提供する git diff の文字数'
				},
				customPrompts: {
					title: 'カスタム AI プロンプト',
					description:
						'GitButler の AI アシスタントはコミットメッセージとブランチ名を生成します。デフォルトのプロンプトを使用するか、独自のプロンプトを作成してください。プロジェクト設定でプロンプトを割り当てます。'
				},
				modelNames: {
					gpt5: 'GPT 5',
					gpt5Mini: 'GPT 5 Mini',
					o3Mini: 'o3 Mini',
					o1Mini: 'o1 Mini',
					gpt4oMini: 'GPT 4o mini',
					gpt41: 'GPT 4.1',
					gpt41Mini: 'GPT 4.1 mini（推奨）',
					haiku: 'Haiku',
					sonnet35: 'Sonnet 3.5',
					sonnet37: 'Sonnet 3.7（推奨）',
					sonnet4: 'Sonnet 4',
					opus4: 'Opus 4'
				}
			},
			telemetry: {
				label: 'テレメトリー',
				description:
					'GitButler はクライアントの改善のためだけにテレメトリーを使用します。以下で明示的に許可されない限り、個人情報は収集しません。<a href="https://gitbutler.com/privacy">プライバシーポリシー</a>',
				request:
					'問題をより迅速に発見できるよう、これらの設定を有効のままにしておいていただけると助かります。無効にする場合は、ぜひ <a href="https://discord.gg/MmFkmaJ42D">Discord</a> でフィードバックをお寄せください。',
				errorReporting: {
					title: 'エラーレポート',
					caption: 'アプリケーションのクラッシュとエラーの報告を切り替えます。'
				},
				usageMetrics: {
					title: '使用状況の指標',
					caption: '使用統計の共有を切り替えます。'
				},
				nonAnonMetrics: {
					title: '非匿名の使用状況指標',
					caption: '識別可能な使用統計の共有を切り替えます。'
				}
			},
			experimental: {
				label: '実験的',
				about:
					'開発中またはベータ版の機能のフラグ。機能が完全に動作しない場合があります。<br />使用は自己責任で。',
				apply3: {
					title: '新しいワークスペースへの適用',
					caption: 'ワークスペースの変更に V3 バージョンの apply と unapply 操作を使用します。'
				},
				fMode: {
					title: 'F モードナビゲーション',
					caption:
						'F モードを有効にして、2文字のショートカットでボタンへの素早いキーボードナビゲーションを実現します。'
				},
				newRebase: {
					title: '新しいリベースエンジン',
					caption: 'スタック操作に新しいグラフベースのリベースエンジンを使用します。'
				},
				singleBranch: {
					title: 'シングルブランチモード',
					caption: 'gitbutler/workspace ブランチを離れるときもワークスペースビューに留まります。'
				},
				irc: {
					title: 'IRC',
					caption: '実験的なアプリ内チャットを有効にします。',
					serverLabel: 'サーバー'
				}
			},
			organizations: {
				label: '組織',
				createButton: '新しい組織を作成'
			},
			footer: {
				social: {
					docs: 'ドキュメント',
					discord: '私たちの Discord'
				}
			}
		},
		project: {
			title: 'プロジェクト設定',
			project: {
				label: 'プロジェクト'
			},
			git: {
				label: 'Git 関連',
				allowForcePush: {
					title: '強制プッシュを許可',
					caption:
						'強制プッシュを許可すると、GitButler はリモートにプッシュ済みでもブランチを上書きできます。GitButler はターゲットブランチに強制プッシュを行うことはありません。'
				},
				forcePushProtection: {
					title: '強制プッシュ保護',
					caption:
						'強制プッシュ中にリモートコミットを保護します。Git のより安全な強制プッシュフラグを使用して、リモートコミット履歴の上書きを回避します。'
				}
			},
			ai: {
				label: 'AI オプション',
				description:
					'GitButler は OpenAI と Anthropic を使用してコミットメッセージとブランチ名の生成をサポートします。これは GitButler の API 経由または独自のキー設定で機能し、メイン設定画面で設定できます。',
				enableGeneration: {
					title: 'ブランチとコミットメッセージの生成を有効化',
					caption:
						'有効にすると、「メッセージを生成」と「ブランチ名を生成」ボタンを押したときに、差分が OpenAI または Anthropic のサーバーに送信されます。'
				},
				enableExperimental: {
					title: '実験的な AI 機能を有効化',
					caption:
						'有効にすると、現在開発中の AI 機能にアクセスできます。これには、機能が機能するために GitButler 経由で OpenAI を使用する必要があります。'
				},
				customPrompts: {
					title: 'カスタムプロンプト',
					description:
						'プロジェクトに独自のカスタムプロンプトを適用できます。デフォルトでは、プロジェクトは GitButler プロンプトを使用しますが、一般設定で独自のプロンプトを作成できます。',
					button: 'プロンプトをカスタマイズ'
				}
			},
			agent: {
				label: 'エージェント',
				guideText:
					'GitButler のエージェントの完全ガイドは<a href="https://docs.gitbutler.com/features/agents-tab#installing-claude-code">ドキュメント</a>でご覧ください',
				autoCommit: {
					title: '完了後に自動コミット',
					caption:
						'Claude Code が完了したら自動的にコミットしてブランチ名を変更します。コミット前に手動で確認する場合は無効にしてください。'
				},
				useConfiguredModel: {
					title: '設定されたモデルを使用',
					caption: '.claude/settings.json で設定されたモデルを使用します。'
				},
				newlineOnEnter: {
					title: 'Enter で改行',
					caption: 'Enter で改行、Cmd+Enter で送信します。'
				},
				notifyOnCompletion: '完了時に通知',
				notifyOnPermissionRequest: '許可が必要なときに通知',
				dangerousPermissions: {
					title: '⚠ 危険：すべての権限を許可',
					caption:
						'すべての許可プロンプトをスキップし、Claude Code に無制限のアクセスを許可します。極めて慎重に使用してください。'
				}
			},
			experimental: {
				label: '実験的',
				ignoreCertificate: {
					title: 'ホスト証明書チェックを無視',
					caption: 'これを有効にすると、ssh で認証するときにホスト証明書チェックが無視されます。'
				}
			},
			details: {
				projectPath: 'プロジェクトパス',
				projectName: 'プロジェクト名',
				projectNamePlaceholder: 'プロジェクト名は空にできません',
				projectDescription: 'プロジェクトの説明',
				runGitHooks: {
					title: 'Git フックを実行',
					caption:
						'これを有効にすると、リポジトリで設定された git pre-push、pre および post commit、commit-msg フックが実行されます。'
				}
			},
			disableCodegen: {
				title: 'コード生成を無効化',
				caption: 'ブランチヘッダーのコード生成ボタンを非表示にします。'
			},
			baseBranch: {
				loading: 'リモートブランチを読み込み中...',
				title: 'リモート設定',
				caption:
					"コードをプッシュする場所と貢献先のターゲットブランチを設定できます。ターゲットブランチは通常、'origin/master' や 'upstream/main' のような「本番」ブランチです。このセクションは、コードが統合用の正しいリモートとブランチに送られることを確認するのに役立ちます。",
				currentTargetBranch: '現在のターゲットブランチ',
				createBranchesOnRemote: 'リモートでブランチを作成',
				activeBranchesWarning:
					'ワークスペースに {count} 個のアクティブブランチがあります。ベースブランチを切り替える前にワークスペースをクリアしてください。',
				switchingBranches: 'ブランチを切り替え中...',
				updateConfiguration: '設定を更新',
				errorListingBranches: 'リモートブランチの一覧表示中にエラーが発生しました'
			},
			remove: {
				title: 'プロジェクトを削除',
				caption: 'プロジェクトを削除しても設定がクリアされるだけで、コードは安全に保たれます。',
				success: 'プロジェクトを削除しました',
				error: 'プロジェクトの削除に失敗しました'
			}
		},
		error: {
			notFound: '設定ページ {id} が見つかりません。'
		}
	}
};

export default locale;
