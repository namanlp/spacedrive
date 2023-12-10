import {
	Books,
	ChartBar,
	Cloud,
	Database,
	FlyingSaucer,
	GearSix,
	HardDrive,
	Key,
	KeyReturn,
	PaintBrush,
	PuzzlePiece,
	Receipt,
	ShieldCheck,
	TagSimple,
	User
} from '@phosphor-icons/react';
import { useFeatureFlag } from '@sd/client';
import { tw } from '@sd/ui';
import { useOperatingSystem } from '~/hooks';
import { usePlatform } from '~/util/Platform';

import Icon from '../Layout/Sidebar/Icon';
import SidebarLink from '../Layout/Sidebar/Link';
import { NavigationButtons } from '../TopBar/NavigationButtons';

const Heading = tw.div`mb-1 ml-1 text-xs font-semibold text-gray-400`;
const Section = tw.div`space-y-0.5`;

export default () => {
	const { platform } = usePlatform();
	const os = useOperatingSystem();

	// const isPairingEnabled = useFeatureFlag('p2pPairing');
	const isBackupsEnabled = useFeatureFlag('backups');

	return (
		<div className="custom-scroll no-scrollbar h-full w-60 max-w-[180px] shrink-0 border-r border-app-line/50 pb-5">
			{platform === 'tauri' ? (
				<div
					data-tauri-drag-region={os === 'macOS'}
					className="mb-3 h-3 w-full p-3 pl-[14px] pt-[10px]"
				>
					<NavigationButtons />
				</div>
			) : (
				<div className="h-3" />
			)}

			<div className="space-y-6 px-4 py-3">
				<Section>
					<Heading>Client</Heading>
					<SidebarLink to="client/general">
						<Icon component={GearSix} />
						General
					</SidebarLink>
					<SidebarLink to="client/usage">
						<Icon component={ChartBar} />
						Usage
					</SidebarLink>
					<SidebarLink to="client/account">
						<Icon component={User} />
						Account
					</SidebarLink>
					<SidebarLink to="node/libraries">
						<Icon component={Books} />
						Libraries
					</SidebarLink>
					<SidebarLink to="client/privacy">
						<Icon component={ShieldCheck} />
						Privacy
					</SidebarLink>
					<SidebarLink to="client/appearance">
						<Icon component={PaintBrush} />
						Appearance
					</SidebarLink>
					<SidebarLink to="client/backups" disabled={!isBackupsEnabled}>
						<Icon component={Database} />
						Backups
					</SidebarLink>
					<SidebarLink to="client/keybindings">
						<Icon component={KeyReturn} />
						Keybinds
					</SidebarLink>
					<SidebarLink to="client/extensions" disabled>
						<Icon component={PuzzlePiece} />
						Extensions
					</SidebarLink>
				</Section>
				<Section>
					<Heading>Library</Heading>
					<SidebarLink to="library/general">
						<Icon component={GearSix} />
						General
					</SidebarLink>
					{/* <SidebarLink to="library/nodes" disabled={!isPairingEnabled}>
						<Icon component={ShareNetwork} />
						Nodes
					</SidebarLink> */}
					<SidebarLink to="library/locations">
						<Icon component={HardDrive} />
						Locations
					</SidebarLink>
					<SidebarLink to="library/tags">
						<Icon component={TagSimple} />
						Tags
					</SidebarLink>
					{/* <SidebarLink to="library/saved-searches">
						<Icon component={MagnifyingGlass} />
						Saved Searches
					</SidebarLink> */}
					<SidebarLink disabled to="library/clouds">
						<Icon component={Cloud} />
						Clouds
					</SidebarLink>
					<SidebarLink to="library/keys" disabled>
						<Icon component={Key} />
						Keys
					</SidebarLink>
				</Section>
				<Section>
					<Heading>Resources</Heading>
					<SidebarLink to="resources/about">
						<Icon component={FlyingSaucer} />
						About
					</SidebarLink>
					<SidebarLink to="resources/changelog">
						<Icon component={Receipt} />
						Changelog
					</SidebarLink>
					{/* <SidebarLink to="resources/dependencies">
						<Icon component={Graph} />
						Dependencies
					</SidebarLink>
					<SidebarLink to="resources/support">
						<Icon component={Heart} />
						Support
					</SidebarLink> */}
				</Section>
			</div>
		</div>
	);
};
