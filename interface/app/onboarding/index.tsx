import { Navigate, redirect, RouteObject } from 'react-router';
import { getOnboardingStore } from '@sd/client';

import Alpha from './alpha';
import { useOnboardingContext } from './context';
import CreatingLibrary from './creating-library';
import { FullDisk } from './full-disk';
import { JoinLibrary } from './join-library';
import Locations from './locations';
import NewLibrary from './new-library';
import Privacy from './privacy';

const Index = () => {
	const obStore = getOnboardingStore();
	const ctx = useOnboardingContext();

	if (obStore.lastActiveScreen && !ctx.library)
		return <Navigate to={obStore.lastActiveScreen} replace />;

	return <Navigate to="alpha" replace />;
};

export default [
	{
		index: true,
		loader: () => {
			if (getOnboardingStore().lastActiveScreen)
				return redirect(`/onboarding/${getOnboardingStore().lastActiveScreen}`, {
					replace: true
				});

			return redirect(`/onboarding/alpha`, { replace: true });
		},
		element: <Index />
	},
	{ path: 'alpha', Component: Alpha },
	// {
	// 	element: <Login />,
	// 	path: 'login'
	// },
	{ Component: NewLibrary, path: 'new-library' },
	{ Component: JoinLibrary, path: 'join-library' },
	{ Component: FullDisk, path: 'full-disk' },
	{ Component: Locations, path: 'locations' },
	{ Component: Privacy, path: 'privacy' },
	{ Component: CreatingLibrary, path: 'creating-library' }
] satisfies RouteObject[];
