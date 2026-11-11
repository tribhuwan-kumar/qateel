import { writable } from 'svelte/store';
import type { Category, ItemMetaData, GidStatus } from '$lib/aria2/types';

import DocumentSvg from "../assets/svgs/documents.svg"
import Everything from "../assets/svgs/everything.svg"
import Music from "../assets/svgs/music.svg"
import Photos from "../assets/svgs/photos.svg"
import Programs from "../assets/svgs/programs.svg"
import Videos from "../assets/svgs/videos.svg"
import Torrent from "../assets/svgs/torrent.svg"
import Subtitles from "../assets/svgs/subtitles.svg"
import Compressed from "../assets/svgs/compressed.svg"
import Other from "../assets/svgs/other.svg"
import EverythingLight from "../assets/svgs/everything-light.svg"
import DocumentLight from "../assets/svgs/documents-light.svg"
import MusicLight from "../assets/svgs/music-light.svg"
import OtherLight from "../assets/svgs/other-light.svg"
import PhotosLight from "../assets/svgs/photos-light.svg"
import ProgramsLight from "../assets/svgs/programs-light.svg"
import SubtitlesLight from "../assets/svgs/subtitles-light.svg"
import TorrentLight from "../assets/svgs/torrent-light.svg"
import VideosLight from "../assets/svgs/videos-light.svg"
import CompressedLight from "../assets/svgs/compressed-light.svg"

export type DownloadCategory = 'everything' | Category;
export type TableStatusFilter = "all" | GidStatus;

export const statusFilters: TableStatusFilter[] = ["all", "active", "paused", "waiting", "complete", "seeding", "removed", "stopped", "error"];
export const selectedCategory = writable<DownloadCategory>('everything');
export const selectedStatus = writable<TableStatusFilter>("all");

export function getItemCategory(item: ItemMetaData): Exclude<DownloadCategory, 'everything'> {
  if (item.category) return item.category;

  const itemName = (item.name ?? "").toLowerCase();
  const itemFiles = (item.files ?? "").toLowerCase();

  if (item.torrent || item.kind === "torrent") return "torrent";

  if (
    /\.(srt|ass|ssa|vtt|sub|idx)(?:$|\?)/.test(itemName) ||
    /\.(srt|ass|ssa|vtt|sub|idx)(?:$|\?)/.test(itemFiles)
  ) return "subtitles";

  if (
    /\.(mp4|mkv|avi|mov|webm|flv|m4v|ts|wmv|3gp|mpeg|mpg)(?:$|\?)/.test(itemName) ||
    /\.(mp4|mkv|avi|mov|webm|flv|m4v|ts|wmv|3gp|mpeg|mpg)(?:$|\?)/.test(itemFiles)
  ) return "videos";

  if (
    /\.(mp3|flac|aac|wav|ogg|m4a|opus|alac|wma)(?:$|\?)/.test(itemName) ||
    /\.(mp3|flac|aac|wav|ogg|m4a|opus|alac|wma)(?:$|\?)/.test(itemFiles)
  ) return "music";

  if (
    /\.(jpg|jpeg|png|gif|webp|bmp|svg|avif|tif|tiff|heic)(?:$|\?)/.test(itemName) ||
    /\.(jpg|jpeg|png|gif|webp|bmp|svg|avif|tif|tiff|heic)(?:$|\?)/.test(itemFiles)
  ) return "photos";

  if (
    /\.(zip|rar|7z|tar|gz|tgz|bz2|xz|zst|lz|lzma|cab)(?:$|\?)/.test(itemName) ||
    /\.(zip|rar|7z|tar|gz|tgz|bz2|xz|zst|lz|lzma|cab)(?:$|\?)/.test(itemFiles)
  ) return "compressed";

  if (
    /\.(exe|msi|apk|appimage|deb|rpm|dmg|pkg|jar|bat|cmd|sh|run|bin)(?:$|\?)/.test(itemName) ||
    /\.(exe|msi|apk|appimage|deb|rpm|dmg|pkg|jar|bat|cmd|sh|run|bin)(?:$|\?)/.test(itemFiles)
  ) return "programs";

  return "others";
}

/// Categorized icons
export const downloadIcons = {
	documents: { dark: DocumentLight, light: DocumentSvg },
	everything: { dark: EverythingLight, light: Everything },
	music: { dark: MusicLight, light: Music },
	other: { dark: OtherLight, light: Other },
	photos: { dark: PhotosLight, light: Photos },
	programs: { dark: ProgramsLight, light: Programs },
	subtitles: { dark: SubtitlesLight, light: Subtitles },
	torrent: { dark: TorrentLight, light: Torrent },
	videos: { dark: VideosLight, light: Videos },
	compressed: { dark: CompressedLight, light: Compressed }
} as const;

/// Return download icons name, based on the mode
export function getDownloadIconName(item: ItemMetaData) {
	if (item.category) {
		if (item.category === "others") return "other";
		return item.category as keyof typeof downloadIcons;
	}

	const itemName = (item.name ?? "").toLowerCase();
	const itemFiles = (item.files ?? "").toLowerCase();

	if (item.torrent || item.kind == "torrent") return "torrent";

	if (
		/\.(srt|ass|ssa|vtt|sub|idx)(?:$|\?)/.test(itemName) ||
		/\.(srt|ass|ssa|vtt|sub|idx)(?:$|\?)/.test(itemFiles)
	) {
		return "subtitles";
	}

	if (
		/\.(mp4|mkv|avi|mov|webm|flv|m4v|ts|wmv|3gp|mpeg|mpg)(?:$|\?)/.test(itemName) ||
		/\.(mp4|mkv|avi|mov|webm|flv|m4v|ts|wmv|3gp|mpeg|mpg)(?:$|\?)/.test(itemFiles)
	) {
		return "videos";
	}

	if (
		/\.(mp3|flac|aac|wav|ogg|m4a|opus|alac|wma)(?:$|\?)/.test(itemName) ||
		/\.(mp3|flac|aac|wav|ogg|m4a|opus|alac|wma)(?:$|\?)/.test(itemFiles)
	) {
		return "music";
	}

	if (
		/\.(jpg|jpeg|png|gif|webp|bmp|svg|avif|tif|tiff|heic)(?:$|\?)/.test(itemName) ||
		/\.(jpg|jpeg|png|gif|webp|bmp|svg|avif|tif|tiff|heic)(?:$|\?)/.test(itemFiles)
	) {
		return "photos";
	}

	if (
		/\.(zip|rar|7z|tar|gz|tgz|bz2|xz|zst|lz|lzma|cab)(?:$|\?)/.test(itemName) ||
		/\.(zip|rar|7z|tar|gz|tgz|bz2|xz|zst|lz|lzma|cab)(?:$|\?)/.test(itemFiles)
	) {
		return "compressed";
	}

	if (
		/\.(pdf|doc|docx|txt|md|rtf|epub|odt|xls|xlsx|ppt|pptx|csv|json|xml|html|htm)(?:$|\?)/.test(itemName) ||
		/\.(pdf|doc|docx|txt|md|rtf|epub|odt|xls|xlsx|ppt|pptx|csv|json|xml|html|htm)(?:$|\?)/.test(itemFiles)
	) {
		return "documents";
	}

	if (
		/\.(exe|msi|apk|appimage|deb|rpm|dmg|pkg|jar|bat|cmd|sh|run|bin)(?:$|\?)/.test(itemName) ||
		/\.(exe|msi|apk|appimage|deb|rpm|dmg|pkg|jar|bat|cmd|sh|run|bin)(?:$|\?)/.test(itemFiles)
	) {
		return "programs";
	}

	return "other";
}

