import { useEffect, useMemo, useRef, useState } from "react";
import Button from "./button";
import { TagInput } from "./components";
import toast from 'react-hot-toast';
import { deepClone, isModKey, SerializableObject, useLocalStorage } from "./helpers";
import { defaultWorkspace, WorkspaceState } from "./workspaces";
import { Bmark } from "./api";
import * as api from './api';

export interface SettingsProps {
    settings: SettingsState;
    tags: string[];
    onSave: (settings: SettingsState) => void;
}

export interface SettingsState extends SerializableObject {
    workspaceState: WorkspaceState;
}

function TagList(props: { allTags: string[], tags: string[], onTagDelete: (tag: string) => void }) {
    function tagExists(tag: string) {
        return props.allTags.includes(tag);
    }

    return <div className="flex flex-col gap-1 overflow-auto">
        {props.tags.map(key => {
            return <div key={key} className="gap-1 flex flex-row">
                <TagInput
                    disabled
                    className={"h-6 text-xs w-full hover:bg-gray-700 " + (tagExists(key) || "bg-red-800 hover:bg-red-600")}
                    excludeDirectMatch
                    onValue={() => null}
                    tagList={props.tags}
                    defaultValue={key}
                />
                <Button
                    className="font-bold bg-red-600 hover:bg-red-700 text-gray-100 text-xs"
                    onClick={() => props.onTagDelete(key)}
                >
                    Delete
                </Button>
            </div>
        })}
    </div>
}

export function Settings(props: SettingsProps) {
    const inputStyle = "transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 text-gray-100";
    const [currWorkspace, setCurrWorkspace] = useState(props.settings.workspaceState.currentWorkspace);

    const workspaceNameRef = useRef<HTMLInputElement | null>();

    const [name, setName] = useState(props.settings.workspaceState.workspaces[currWorkspace].name);

    const [loading, setLoading] = useState(true);

    const [settings, setSettings] = useState(deepClone(props.settings));

    const [newWhiteTag, setNewWhiteTag] = useState("");
    const [newBlackTag, setNewBlackTag] = useState("");

    const [allBmarks, setAllBmarks] = useState<Bmark[]>([]);

    const [relatedTagsUpdateKey, _updateRelatedTags] = useState(0);
    function updateRelatedTags() {
        _updateRelatedTags(Math.random());
    }

    const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
        if (e.key === "Enter" && !isModKey(e)) {
            e.preventDefault();
            onSave();
        }
    }

    useEffect(() => {
        setSettings(deepClone(props.settings));
    }, [props.settings]);

    const onTagDelete = (tag: string) => {
        props.onSave(settings);
    }

    const onSave = (settingsOpt?: SettingsState) => {
        const s = settingsOpt ?? settings;
        const currWorkspaceName = s.workspaceState.workspaces[s.workspaceState.currentWorkspace].name;

        if (s.workspaceState.workspaces.length === 0) {
            toast.error("At least one workspace is required");
            return
        }

        s.workspaceState.workspaces.forEach((ws, idx) => {
            if (ws.name.trim() === "") {
                toast.error("Workspace name cannot be empty");
                return
            }
            if (ws.tags.whitelist.find(t => t.trim() === "")) {
                toast.error("Tag cannot be empty");
                return
            }
            if (ws.tags.blacklist.find(t => t.trim() === "")) {
                toast.error("Tag cannot be empty");
                return
            }
        });

        s.workspaceState.workspaces.sort((a, b) => a.name.localeCompare(b.name));

        s.workspaceState.currentWorkspace = s.workspaceState.workspaces.findIndex(ws => ws.name === currWorkspaceName);

        props.onSave(s);
    };

    function createWorkspace() {
        const workspace = defaultWorkspace();
        settings.workspaceState.workspaces.push(workspace);

        workspaceNameRef.current?.focus();
        workspaceNameRef.current?.setSelectionRange(0, workspace.name.length);

        onSave();
        setCurrWorkspace(settings.workspaceState.workspaces.findIndex(ws => ws.name === workspace.name));
        setName(workspace.name);
    }

    function deleteWorkspace() {
        settings.workspaceState.workspaces.splice(currWorkspace, 1);
        setCurrWorkspace(0);
        onSave();
    }

    function renameWorkspace() {
        settings.workspaceState.workspaces[currWorkspace].name = name;
        onSave();

        setCurrWorkspace(settings.workspaceState.workspaces.findIndex(ws => ws.name === name));
    }

    function selectWorkspace(idx: number) {
        setCurrWorkspace(idx);
        setName(settings.workspaceState.workspaces[idx].name);
    }

    function addWhitelistTag(tag: string) {
        const { blacklist, whitelist } = settings.workspaceState.workspaces[currWorkspace].tags;
        setTimeout(() => {
            (document.querySelector("#whitelist-tag-input") as HTMLInputElement)?.focus?.();
        });
        if (tag.trim() === "") {
            toast.error("Tag cannot be empty");
            return
        }
        if (whitelist.includes(tag) || blacklist.includes(tag)) {
            toast.error("Tag already exists");
            return
        }

        settings.workspaceState.workspaces[currWorkspace].tags.whitelist.unshift(tag);
        onSave();

        setNewWhiteTag("");

    }

    function addBlacklistTag(tag: string) {
        const { blacklist, whitelist } = settings.workspaceState.workspaces[currWorkspace].tags;
        setTimeout(() => {
            (document.querySelector("#blacklist-tag-input") as HTMLInputElement)?.focus?.();
        });
        if (tag.trim() === "") {
            toast.error("Tag cannot be empty");
            return
        }
        if (blacklist.includes(tag) || whitelist.includes(tag)) {
            toast.error("Tag already exists");
            return
        }

        settings.workspaceState.workspaces[currWorkspace].tags.blacklist.unshift(tag);
        onSave();

        setNewBlackTag("");

    }

    function removeTag(tag: string) {
        settings.workspaceState.workspaces[currWorkspace].tags.blacklist = settings.workspaceState.workspaces[currWorkspace].tags.blacklist.filter(t => t !== tag);
        settings.workspaceState.workspaces[currWorkspace].tags.whitelist = settings.workspaceState.workspaces[currWorkspace].tags.whitelist.filter(t => t !== tag);

        onSave();
    }

    useEffect(() => {
        api.fetchBmarks({ descending: true }).then(bmarks => {
            setLoading(false);
            if (relatedTagsUpdateKey > 0) {
                setAllBmarks(bmarks);
            }
        });
    }, [relatedTagsUpdateKey]);

    function findRelatedTags(bmarks: Bmark[], tagsSubset: string[]) {
        const relatedTags = new Set<string>();

        const bmarksFiltered = bmarks.filter(bmark => bmark.tags.some(t => tagsSubset.find(ts => t === ts)));
        for (const bmark of bmarksFiltered) {
            for (const t of bmark.tags) {
                if (tagsSubset.includes(t)) {
                    continue
                }
                relatedTags.add(t);
            }
        }

        const out = Array.from(relatedTags);

        out.sort((a, b) => a.localeCompare(b));

        return out;
    }

    const relatedTagsWhitelist = useMemo(() => {
        if (!allBmarks.length) {
            return []
        }

        const workspace = settings.workspaceState.workspaces[currWorkspace];
        const whitelist = workspace.tags.whitelist.filter(t => t !== "untagged");
        if (whitelist.length === 0) {
            return []
        }

        return findRelatedTags(allBmarks, whitelist);
    }, [allBmarks, relatedTagsUpdateKey, currWorkspace]);

    const relatedTagsBlacklist = useMemo(() => {
        if (!allBmarks.length) {
            return []
        }

        const workspace = settings.workspaceState.workspaces[currWorkspace];
        const blacklist = workspace.tags.blacklist.filter(t => t !== "untagged");
        console.log(workspace.name, blacklist)
        if (blacklist.length === 0) {
            return []
        }
        return findRelatedTags(allBmarks, blacklist);

    }, [allBmarks, relatedTagsUpdateKey, currWorkspace]);

    if (loading) {
        return <div className={"bmark-container relative text-wrap break-words p-6 h-screen"}>
            <div
                className={"bg-gray-800 rounded-lg h-full overflow-hidden flex flex-col"}
            >
                <div className="flex flex-col items-center justify-center h-full">
                    <div className="animate-spin rounded-full h-32 w-32 border-t-2 border-b-2 border-gray-700"></div>
                </div>
            </div>
        </div>
    }

    function saveSettings() {
        const blob = new Blob([JSON.stringify(settings)], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = "settings.json";
        a.click();

        a.remove();
    }

    return <div
        className={"bg-gray-800 rounded-lg flex flex-col h-full"}
    >
        <div className="text-gray-100 font-bold px-3 py-2 mb-5 text-xl">
            Workspaces
        </div>
        <div className="px-3 flex flex-row gap-1 mb-4 w-full">
            <div className="flex gap-1">
                <Button
                    className=" h-full px-4 py-1 mb-3 font-bold bg-sky-600 hover:bg-sky-700 text-gray-100"
                    onClick={createWorkspace}
                >
                    +
                </Button>
                <Button
                    className=" h-full px-4 py-1 mb-3 font-bold bg-pink-600 hover:bg-pink-700 text-gray-100"
                    onClick={deleteWorkspace}
                >
                    -
                </Button>
            </div>
            <div className="flex">
                <select
                    className="transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 max-w-64 border-none"
                    value={currWorkspace}
                    onChange={e => selectWorkspace(parseInt(e.currentTarget.value))}
                >
                    {settings.workspaceState.workspaces.map((ws, idx) => <option key={idx} value={idx}>{ws.name}</option>)}
                </select>
            </div>
            <div className="flex">
                <input
                    className="transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 max-w-64 border-none"
                    type="text"
                    value={name}
                    onInput={e => setName(e.currentTarget.value)}
                    ref={ref => workspaceNameRef.current = ref}
                />
            </div>
            <div className="flex gap-1">
                {name !== settings.workspaceState.workspaces[currWorkspace]?.name && <Button
                    className="px-4 py-1 font-bold bg-green-600 hover:bg-green-700 text-gray-100"
                    onClick={renameWorkspace}
                    disabled={name.trim() === ""}
                >
                    Rename
                </Button>}
                {name !== settings.workspaceState.workspaces[currWorkspace].name && <Button
                    className="px-4 py-1 font-bold bg-pink-600 hover:bg-pink-700 text-gray-100"
                    onClick={() => setName(settings.workspaceState.workspaces[currWorkspace].name)}
                >
                    Reset
                </Button>}
            </div>
        </div>

        <div className="flex flex-row gap-2 w-full justify-around flex-1 overflow-auto">
            <div className="flex flex-col h-full">
                <div className="text-gray-100 font-bold px-3 py-2 text-xl flex-none basis-1/12">
                    Whitelist
                </div>
                <div className="flex flex-col gap-3 p-1 h-full basis-11/12 overflow-hidden">
                    <div className="flex flex-row gap-1  basis-auto" key={JSON.stringify(settings)}>
                        <TagInput
                            excludeDirectMatch
                            single
                            id="whitelist-tag-input"
                            onValue={(tags) => setNewWhiteTag(tags[0])}
                            defaultValue={newWhiteTag}
                            tagList={props.tags.filter(t => {
                                return !settings.workspaceState.workspaces[currWorkspace].tags.blacklist.includes(t)
                                    && !settings.workspaceState.workspaces[currWorkspace].tags.whitelist.includes(t)
                            })}
                            onKeyDown={e => {
                                if (e.key === "Enter") {
                                    e.preventDefault();
                                    addWhitelistTag(newWhiteTag);
                                }
                            }}
                        />
                        <Button
                            className="px-4 py-1 font-bold bg-green-600 hover:bg-green-700 text-gray-100"
                            onClick={() => addWhitelistTag(newWhiteTag)}
                            disabled={newWhiteTag.trim() === ""}
                        >
                            Add
                        </Button>
                    </div>
                    <div className="flex flex-col overflow-auto py-2  h-full">
                        <TagList allTags={props.tags} tags={settings.workspaceState.workspaces[currWorkspace].tags.whitelist} onTagDelete={tag => removeTag(tag)} />
                    </div>
                    <div className="grid grid-cols-4 grid-rows-10 gap-[2px] overflow-x-hidden overflow-y-auto py-2 basis-4/12">
                        {relatedTagsWhitelist.map(tag => <div key={tag} className="flex flex-row text-white">
                            <div onClick={() => addWhitelistTag(tag)} className="px-1 text-xs rounded cursor-pointer select-none transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500">{tag}</div>
                        </div>)}
                    </div>
                </div>
            </div>

            <div className="flex flex-col h-full">
                <div className="text-gray-100 font-bold px-3 py-2 text-xl flex-none basis-1/12">
                    Blacklist
                </div>
                <div className="flex flex-col gap-3 p-1 h-full basis-11/12 overflow-hidden">
                    <div className="flex flex-row gap-1  basis-auto" key={JSON.stringify(settings)}>
                        <TagInput
                            excludeDirectMatch
                            single
                            id="blacklist-tag-input"
                            onValue={(tags) => setNewBlackTag(tags[0])}
                            defaultValue={newBlackTag}
                            tagList={props.tags.filter(t => {
                                return !settings.workspaceState.workspaces[currWorkspace].tags.blacklist.includes(t)
                                    && !settings.workspaceState.workspaces[currWorkspace].tags.blacklist.includes(t)
                            })}
                            onKeyDown={e => {
                                if (e.key === "Enter") {
                                    e.preventDefault();
                                    addBlacklistTag(newBlackTag);
                                }
                            }}
                        />
                        <Button
                            className="px-4 py-1 font-bold bg-green-600 hover:bg-green-700 text-gray-100"
                            onClick={() => addBlacklistTag(newBlackTag)}
                            disabled={newBlackTag.trim() === ""}
                        >
                            Add
                        </Button>
                    </div>
                    <div className="flex flex-col overflow-auto py-2  h-full">
                        <TagList allTags={props.tags} tags={settings.workspaceState.workspaces[currWorkspace].tags.blacklist} onTagDelete={tag => removeTag(tag)} />
                    </div>
                    <div className="grid grid-cols-4 gap-1 overflow-x-hidden overflow-y-auto py-2 basis-4/12">
                        {relatedTagsBlacklist.map(tag => <div key={tag} className="flex flex-row text-white">
                            <div onClick={() => addBlacklistTag(tag)} className="px-1 text-xs rounded cursor-pointer select-none transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500">{tag}</div>
                        </div>)}
                    </div>

                </div>
            </div>

        </div>
        <div className="px-3 flex flex-row gap-1 mb-1 w-full pt-4">
            <Button
                className="mb-1 font-bold bg-green-600 hover:bg-green-700 text-gray-100 w-full"
                onClick={() => updateRelatedTags()}
            >
                Fetch related
            </Button>
        </div>

        <div className="px-3 flex flex-row gap-1 mb-4 w-full pt-1">
            <div className="ml-auto flex">
                <input
                    className="transition-all bg-gray-700 hover:bg-gray-600/90 focus:bg-gray-500 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-2 max-w-64 border-none"
                    type="file"
                    onChange={e => {
                        const file = e.currentTarget.files?.item(0);
                        if (!file) {
                            toast.error("No file selected");
                            return
                        }
                        const reader = new FileReader();
                        reader.onload = e => {
                            try {
                                const settings = JSON.parse(e.target?.result as string);
                                if (settings.workspaceState) {
                                    setSettings(settings);
                                    onSave(settings);
                                } else {
                                    toast.error("Invalid settings.json file");
                                }
                            } catch (err) {
                                toast.error("Invalid settings.json file: " + err);
                            }
                        };
                        reader.readAsText(file);
                    }}
                />
            </div>
            <div className="flex">
                <Button
                    className="bg-sky-600 hover:bg-sky-700 text-gray-100"
                    onClick={() => saveSettings()}
                >
                    Export
                </Button>
            </div>
        </div>
    </div>
}

export default Settings;
