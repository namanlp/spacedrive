import { createMemoryHistory } from '@remix-run/router';
import { QueryClientProvider } from '@tanstack/react-query';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { startTransition, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { CacheProvider, createCache, RspcProvider } from '@sd/client';
import {
	createRoutes,
	ErrorPage,
	KeybindEvent,
	PlatformProvider,
	SpacedriveInterfaceRoot,
	SpacedriveRouterProvider,
	TabsContext
} from '@sd/interface';
import { RouteTitleContext } from '@sd/interface/hooks/useRouteTitle';
import { getSpacedropState } from '@sd/interface/hooks/useSpacedropState';

import '@sd/ui/style/style.scss';

import { commands } from './commands';
import { platform } from './platform';
import { queryClient } from './query';
import { createMemoryRouterWithHistory } from './router';

// TODO: Bring this back once upstream is fixed up.
// const client = hooks.createClient({
// 	links: [
// 		loggerLink({
// 			enabled: () => getDebugState().rspcLogger
// 		}),
// 		tauriLink()
// 	]
// });

const startupError = (window as any).__SD_ERROR__ as string | undefined;

export default function App() {
	useEffect(() => {
		// This tells Tauri to show the current window because it's finished loading
		commands.appReady();
	}, []);

	useEffect(() => {
		const keybindListener = listen('keybind', (input) => {
			document.dispatchEvent(new KeybindEvent(input.payload as string));
		});

		const dropEventListener = appWindow.onFileDropEvent((event) => {
			if (event.payload.type === 'drop') {
				getSpacedropState().droppedFiles = event.payload.paths;
			}
		});

		return () => {
			keybindListener.then((unlisten) => unlisten());
			dropEventListener.then((unlisten) => unlisten());
		};
	}, []);

	return (
		<RspcProvider queryClient={queryClient}>
			<PlatformProvider platform={platform}>
				<QueryClientProvider client={queryClient}>
					<CacheProvider cache={cache}>
						{startupError ? (
							<ErrorPage
								message={startupError}
								submessage="Error occurred starting up the Spacedrive core"
							/>
						) : (
							<AppInner />
						)}
					</CacheProvider>
				</QueryClientProvider>
			</PlatformProvider>
		</RspcProvider>
	);
}

// we have a minimum delay between creating new tabs as react router can't handle creating tabs super fast
const TAB_CREATE_DELAY = 150;

const cache = createCache();

const routes = createRoutes(platform, cache);

function AppInner() {
	const [tabs, setTabs] = useState(() => [createTab()]);
	const [tabIndex, setTabIndex] = useState(0);

	function createTab() {
		const history = createMemoryHistory();
		const router = createMemoryRouterWithHistory({ routes, history });

		const dispose = router.subscribe((event) => {
			// we don't care about non-idle events as those are artifacts of form mutations + suspense
			if (event.navigation.state !== 'idle') return;

			setTabs((routers) => {
				const index = routers.findIndex((r) => r.router === router);
				if (index === -1) return routers;

				const routerAtIndex = routers[index]!;

				routers[index] = {
					...routerAtIndex,
					currentIndex: history.index,
					maxIndex:
						event.historyAction === 'PUSH'
							? history.index
							: Math.max(routerAtIndex.maxIndex, history.index)
				};

				return [...routers];
			});
		});

		return {
			id: Math.random().toString(),
			router,
			history,
			dispose,
			element: document.createElement('div'),
			currentIndex: 0,
			maxIndex: 0,
			title: 'New Tab'
		};
	}

	const tab = tabs[tabIndex]!;

	const createTabPromise = useRef(Promise.resolve());

	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		const div = ref.current;
		if (!div) return;

		div.appendChild(tab.element);

		return () => {
			while (div.firstChild) {
				div.removeChild(div.firstChild);
			}
		};
	}, [tab.element]);

	return (
		<RouteTitleContext.Provider
			value={useMemo(
				() => ({
					setTitle(title) {
						setTabs((oldTabs) => {
							const tabs = [...oldTabs];
							const tab = tabs[tabIndex];
							if (!tab) return tabs;

							tabs[tabIndex] = { ...tab, title };

							return tabs;
						});
					}
				}),
				[tabIndex]
			)}
		>
			<TabsContext.Provider
				value={{
					tabIndex,
					setTabIndex,
					tabs: tabs.map(({ router, title }) => ({ router, title })),
					createTab() {
						createTabPromise.current = createTabPromise.current.then(
							() =>
								new Promise((res) => {
									startTransition(() => {
										setTabs((tabs) => {
											const newTabs = [...tabs, createTab()];

											setTabIndex(newTabs.length - 1);

											return newTabs;
										});
									});

									setTimeout(res, TAB_CREATE_DELAY);
								})
						);
					},
					removeTab(index: number) {
						startTransition(() => {
							setTabs((tabs) => {
								const tab = tabs[index];
								if (!tab) return tabs;

								tab.dispose();

								tabs.splice(index, 1);

								setTabIndex(Math.min(tabIndex, tabs.length - 1));

								return [...tabs];
							});
						});
					}
				}}
			>
				<SpacedriveInterfaceRoot>
					{tabs.map((tab) =>
						createPortal(
							<SpacedriveRouterProvider
								key={tab.id}
								routing={{
									routes,
									visible: tabIndex === tabs.indexOf(tab),
									router: tab.router,
									currentIndex: tab.currentIndex,
									maxIndex: tab.maxIndex
								}}
							/>,
							tab.element
						)
					)}
					<div ref={ref} />
				</SpacedriveInterfaceRoot>
			</TabsContext.Provider>
		</RouteTitleContext.Provider>
	);
}
