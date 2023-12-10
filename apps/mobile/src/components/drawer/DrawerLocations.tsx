import { DrawerNavigationHelpers } from '@react-navigation/drawer/lib/typescript/src/types';
import { useNavigation } from '@react-navigation/native';
import { useRef } from 'react';
import { Pressable, Text, View } from 'react-native';
import { useCache, useLibraryQuery, useNodes } from '@sd/client';
import { ModalRef } from '~/components/layout/Modal';
import { tw, twStyle } from '~/lib/tailwind';

import FolderIcon from '../icons/FolderIcon';
import CollapsibleView from '../layout/CollapsibleView';
import ImportModal from '../modal/ImportModal';

type DrawerLocationItemProps = {
	folderName: string;
	onPress: () => void;
};

const DrawerLocationItem: React.FC<DrawerLocationItemProps> = (props) => {
	const { folderName, onPress } = props;

	return (
		<Pressable onPress={onPress}>
			<View style={twStyle('mb-[4px] flex flex-row items-center rounded px-1 py-2')}>
				<FolderIcon size={20} />
				<Text style={twStyle('ml-1.5 font-medium text-gray-300')} numberOfLines={1}>
					{folderName}
				</Text>
			</View>
		</Pressable>
	);
};

type DrawerLocationsProp = {
	stackName: string;
};

const DrawerLocations = ({ stackName }: DrawerLocationsProp) => {
	const navigation = useNavigation<DrawerNavigationHelpers>();

	const modalRef = useRef<ModalRef>(null);

	const result = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	useNodes(result.data?.nodes);
	const locations = useCache(result.data?.items);

	return (
		<>
			<CollapsibleView
				title="Locations"
				titleStyle={tw`text-sm font-semibold text-gray-300`}
				containerStyle={tw`mb-3 ml-1 mt-6`}
			>
				<View style={tw`mt-2`}>
					{locations?.map((location) => (
						<DrawerLocationItem
							key={location.id}
							folderName={location.name ?? ''}
							onPress={() =>
								navigation.navigate(stackName, {
									screen: 'Location',
									params: { id: location.id }
								})
							}
						/>
					))}
				</View>
				{/* Add Location */}
				<Pressable onPress={() => modalRef.current?.present()}>
					<View style={tw`mt-1 rounded border border-dashed border-app-line/80`}>
						<Text style={tw`p-2 text-center text-xs font-bold text-gray-400`}>
							Add Location
						</Text>
					</View>
				</Pressable>
			</CollapsibleView>
			<ImportModal ref={modalRef} />
		</>
	);
};

export default DrawerLocations;
