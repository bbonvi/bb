import { useEffect, useRef, useState } from "react";
import { Config } from "./api";
import Button from "./button";
import { TagInput } from "./components";
import AutosizeInput from 'react-input-autosize';

interface HeaderProps {
    tags: string;
    title: string;
    url: string;
    description: string;

    count: number;
    total: number;

    tagList: string[];

    columns: number;
    onColumns: (val: number) => void;

    onTags: (val: string) => void;
    onTitle: (val: string) => void;
    onUrl: (val: string) => void;
    onDescription: (val: string) => void;

    onRef: (ref: HTMLDivElement | null) => void;

    onNewBookmark: () => void;

    config: Config;
    hiddenByDefault: string[];

    setIgnoreHidden: (val: boolean) => void;
    ignoreHidden: boolean;
    setShuffle: (val: boolean) => void;
    shuffle: boolean;
}

function Header(props: HeaderProps) {
    const [loaded, setLoaded] = useState(false);
    const [saveQueries, _setSaveQuries] = useState(localStorage["saveQueries"] === "true");
    const setSaveQuries = (val: boolean) => {
        _setSaveQuries(val);
        localStorage["saveQueries"] = JSON.stringify(val)
    }

    function setQuery(tags: string, title: string, url: string, description: string) {
        const urlParams = new URL(window.location.href);
        if (tags) {
            urlParams.searchParams.set("tags", tags);
        } else {
            urlParams.searchParams.delete("tags");
        }

        if (url) {
            urlParams.searchParams.set("url", url);
        } else {
            urlParams.searchParams.delete("url");
        }

        if (description) {
            urlParams.searchParams.set("description", description);
        } else {
            urlParams.searchParams.delete("description");
        }

        if (title) {
            urlParams.searchParams.set("title", title);
        } else {
            urlParams.searchParams.delete("title");
        }

        window.history.pushState({}, "", urlParams);
    }

    const onTitle = (val: string) => {
        if (saveQueries) {
            setQuery(props.tags, val, props.url, props.description)
        }

        props.onTitle(val);
    }

    const onTags = (val: string) => {
        if (saveQueries) {
            setQuery(val, props.title, props.url, props.description)
        }

        props.onTags(val);
    }

    const onUrl = (val: string) => {
        if (saveQueries) {
            setQuery(props.tags, props.title, val, props.description)
        }

        props.onUrl(val);
    }

    const onDescription = (val: string) => {
        if (saveQueries) {
            setQuery(props.tags, props.title, props.url, val)
        }

        props.onDescription(val);
    }

    useEffect(() => {
        const defaultTags = new URLSearchParams(window.location.search).get("tags") ?? "";
        const defaultTitle = new URLSearchParams(window.location.search).get("title") ?? "";
        const defaultUrl = new URLSearchParams(window.location.search).get("url") ?? "";
        const defaultDescription = new URLSearchParams(window.location.search).get("description") ?? "";
        const ignoreHidden = new URLSearchParams(window.location.search).get("ignore_hidden") ?? "";

        setQuery(defaultTags, defaultTitle, defaultUrl, defaultDescription)
        props.onTitle(defaultTitle);
        props.onTags(defaultTags);
        props.onDescription(defaultDescription);
        props.onUrl(defaultUrl);
        props.setIgnoreHidden(ignoreHidden === "1" || ignoreHidden === "true");

        setLoaded(true)
    }, [])

    const inputTextClassNames = "transition-all bg-gray-800/60 hover:bg-gray-800/90 focus:bg-gray-600/60 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-3 max-w-96 ";

    const total = props.total >= 0 ? props.total.toString() : "...";
    const count = props.count >= 0 ? props.count.toString() : "...";

    if (!loaded) {
        return <div />
    }

    return <div
        ref={ref => props.onRef(ref)}
        className="header top-0 left-0 right-0 fixed z-40 bg-gray-900 motion-safe:bg-gray-900/80 motion-safe:backdrop-blur-2xl p-5 shadow-lg flex flex-wrap gap-2"
    >
        <TagInput
            isSearch
            listenEvent
            autoSize
            hiddenTags={props.hiddenByDefault}
            onValue={(value) => onTags(value.join(" "))}
            tagList={props.tagList}
            defaultValue={props.tags}
            className="bg-gray-800/60 hover:bg-gray-800/90 focus:bg-gray-600/60 tag-search"
        />
        <AutosizeInput
            onInput={e => onTitle(e.currentTarget.value)}
            type="text"
            value={props.title}
            extraWidth={10}
            placeholderIsMinWidth
            placeholder="Title"
            className="!flex"
            inputClassName={inputTextClassNames + "auto-size"}
        />
        <AutosizeInput
            extraWidth={10}
            onInput={e => onUrl(e.currentTarget.value)}
            type="text"
            value={props.url}
            placeholderIsMinWidth
            placeholder="Url"
            className="!flex"
            inputClassName={inputTextClassNames + "auto-size"}
        />
        <AutosizeInput
            onInput={e => onDescription(e.currentTarget.value)}
            type="text"
            extraWidth={10}
            placeholderIsMinWidth
            value={props.description}
            placeholder="Description"
            className="!flex"
            inputClassName={inputTextClassNames + "auto-size"}
        />
        {/*<span
            contentEditable
            onInput={e => onDescription(e.currentTarget.textContent ?? "")}
            content={props.description}
            className={inputTextClassNames}
        />*/}
        <div className="text-sm flex"><span className="my-auto">{count}/{total}</span></div>
        <div className="text-sm flex"></div>
        <div className="text-sm flex"><Button onClick={() => props.onNewBookmark()} className="my-auto font-bold bg-green-600 hover:bg-green-700  px-3 py-1">New</Button></div>
        <div className="ml-auto text-sm flex">
            <div className="flex align-sub my-auto">
                <div className="text-gray-100 flex mr-2" style={{ flexDirection: "column" }}>
                    <div className="text-gray-100 flex mr-2" title="Auto save search queries in url">
                        <input
                            id="save-queries"
                            onChange={e => setSaveQuries(e.currentTarget.checked)}
                            type="checkbox"
                            autoComplete="off"
                            autoCorrect="off"
                            checked={saveQueries}
                            placeholder="Tags"
                            className={inputTextClassNames + " w-auto mr-2 text-left cursor-pointer"}
                        />
                        <label htmlFor="save-queries" className="w-full cursor-pointer">Save queries</label>
                    </div>
                    <div className="text-gray-100 flex mr-2" title="Ignore hidden tags">
                        <input
                            id="fetch-meta"
                            onChange={e => props.setIgnoreHidden(!props.ignoreHidden)}
                            type="checkbox"
                            autoComplete="off"
                            autoCorrect="off"
                            checked={props.ignoreHidden}
                            placeholder="Tags"
                            className={inputTextClassNames + " w-auto mr-2 text-left cursor-pointer"}
                        />
                        <label htmlFor="fetch-meta" className="w-full cursor-pointer">Show all</label>
                    </div>
                    <div className="text-gray-100 flex mr-2" title="Shuffle">
                        <input
                            onChange={e => props.setShuffle(!props.shuffle)}
                            type="checkbox"
                            id="shuffle"
                            autoComplete="off"
                            autoCorrect="off"
                            checked={props.shuffle}
                            placeholder="Shuffle"
                            className={inputTextClassNames + " w-auto mr-2 text-left cursor-pointer"}
                        />
                        <label htmlFor="shuffle" className="w-full cursor-pointer">Shuffle</label>
                    </div>
                </div>

                <div className="text-gray-100 flex mr-2">
                    <div className="my-auto"><Button onClick={() => props.onColumns(Math.max(1, props.columns - 1))} >-</Button></div>
                    <div className="mx-1 my-auto">{props.columns}</div>
                    <div className="my-auto"><Button onClick={() => props.onColumns(Math.min(12, props.columns + 1))}>+</Button></div>
                </div>
            </div>


        </div>
    </div>
}

export default Header;
