import { iconNames } from '@sd/assets/util';
import { memo, useEffect, useMemo, useState } from 'react';
import { byteSize, useDiscoveredPeers, useLibraryQuery, useNodes } from '@sd/client';
import { Card } from '@sd/ui';
import { Icon } from '~/components';
import { useCounter } from '~/hooks';

import { Heading } from '../Layout';

export const Component = () => {
	const stats = useLibraryQuery(['library.statistics'], {
		refetchOnWindowFocus: false,
		initialData: { total_bytes_capacity: '0', library_db_size: '0' }
	});
	const locations = useLibraryQuery(['locations.list'], {
		refetchOnWindowFocus: false
	});
	useNodes(locations.data?.nodes);
	// const locations = useCache(result.data?.items);

	const discoveredPeers = useDiscoveredPeers();
	const info = useMemo(() => {
		if (locations.data && discoveredPeers) {
			const tb_capacity = byteSize(stats.data?.total_bytes_capacity);
			const free_space = byteSize(stats.data?.total_bytes_free);
			const library_db_size = byteSize(stats.data?.library_db_size);
			const preview_media = byteSize(stats.data?.preview_media_bytes);
			const data: {
				icon: keyof typeof iconNames;
				title?: string;
				numberTitle?: number;
				titleCount?: number;
				unit?: string;
				sub: string;
				dataLength?: number;
			}[] = [
				{
					icon: 'Folder',
					title: locations.data?.items.length === 1 ? 'Location' : 'Locations',
					titleCount: locations.data?.items.length ?? 0,
					sub: 'indexed directories'
				},
				{
					icon: 'Laptop',
					title: discoveredPeers.size >= 0 ? 'Devices' : 'Device',
					titleCount: discoveredPeers.size ?? 0,
					sub: 'in your network'
				},
				{
					icon: 'DriveDarker',
					numberTitle: tb_capacity.value,
					sub: 'Total capacity',
					unit: tb_capacity.unit
				},
				{
					icon: 'HDD',
					numberTitle: free_space.value,
					sub: 'Free space',
					unit: free_space.unit
				},
				{
					icon: 'Collection',
					numberTitle: library_db_size.value,
					sub: 'Library size',
					unit: library_db_size.unit
				},
				{
					icon: 'Image',
					numberTitle: preview_media.value,
					sub: 'Preview media',
					unit: preview_media.unit
				}
			];
			return data;
		}
	}, [locations, discoveredPeers, stats]);

	return (
		<>
			<Heading title="Usage" description="Your library usage and hardware information" />
			<Card className="flex w-full flex-col justify-center !p-5">
				<div className="grid grid-cols-1 justify-center gap-2 lg:grid-cols-2 xl:grid-cols-3">
					{info?.map((i, index) => (
						<UsageCard
							key={index}
							icon={i.icon}
							title={i.title as string}
							numberTitle={i.numberTitle}
							titleCount={i.titleCount as number}
							statsLoading={stats.isLoading}
							unit={i.unit}
							sub={i.sub}
						/>
					))}
				</div>
			</Card>
		</>
	);
};

interface Props {
	icon: keyof typeof iconNames;
	title: string;
	titleCount?: number;
	numberTitle?: number;
	statsLoading: boolean;
	unit?: string;
	sub: string;
}

let mounted = false;
const UsageCard = memo(
	({ icon, title, titleCount, numberTitle, unit, sub, statsLoading }: Props) => {
		const [isMounted] = useState(mounted);
		const sizeCount = useCounter({
			name: title,
			end: Number(numberTitle ? numberTitle : titleCount),
			duration: isMounted ? 0 : 1,
			precision: numberTitle ? 1 : 0,
			saveState: false
		});
		useEffect(() => {
			if (!statsLoading) mounted = true;
		});

		return (
			<Card className="h-fit w-full bg-app-input py-4">
				<div className="flex w-full items-center justify-center gap-3">
					<Icon name={icon} size={40} />
					<div className="w-full max-w-[120px]">
						<h1 className="text-lg font-medium">
							{typeof titleCount === 'number' && (
								<span className="mr-1 text-ink-dull">{sizeCount}</span>
							)}
							{numberTitle && sizeCount}
							{title}
							{unit && (
								<span className="ml-1 text-[16px] font-normal text-ink-dull">
									{unit}
								</span>
							)}
						</h1>
						<p className="text-sm text-ink-faint">{sub}</p>
					</div>
				</div>
			</Card>
		);
	}
);
