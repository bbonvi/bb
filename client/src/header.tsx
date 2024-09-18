import { useEffect, useRef, useState } from "react";
import { Config } from "./api";
import Button from "./button";
import { TagInput } from "./components";

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
}

function Header(props: HeaderProps) {
    const [loaded, setLoaded] = useState(false);
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
        setQuery(props.tags, val, props.url, props.description)

        props.onTitle(val);
    }

    const onTags = (val: string) => {
        setQuery(val, props.title, props.url, props.description)

        props.onTags(val);
    }

    const onUrl = (val: string) => {
        setQuery(props.tags, props.title, val, props.description)

        props.onUrl(val);
    }

    const onDescription = (val: string) => {
        setQuery(props.tags, props.title, props.url, val)

        props.onDescription(val);
    }

    useEffect(() => {
        const defaultTags = new URLSearchParams(window.location.search).get("tags") ?? "";
        const defaultTitle = new URLSearchParams(window.location.search).get("title") ?? "";
        const defaultUrl = new URLSearchParams(window.location.search).get("url") ?? "";
        const defaultDescription = new URLSearchParams(window.location.search).get("description") ?? "";

        setQuery(defaultTags, defaultTitle, defaultUrl, defaultDescription)
        props.onTitle(defaultTitle);
        props.onTags(defaultTags);
        props.onDescription(defaultDescription);
        props.onUrl(defaultUrl);

        setLoaded(true)
    }, [])

    const inputTextClassNames = "transition-all bg-gray-800/60 hover:bg-gray-800/90 focus:bg-gray-600/60 shadow-sm hover:shadow-inner focus:shadow-inner text-gray-100 rounded outline-0 p-1 px-3";

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
            hiddenTags={props.config.hidden_by_default}
            onValue={(value) => onTags(value.join(" "))}
            tagList={props.tagList}
            defaultValue={props.tags}
            className="bg-gray-800/60 hover:bg-gray-800/90 focus:bg-gray-600/60 tag-search"
        />
        <input
            onInput={e => onTitle(e.currentTarget.value)}
            type="text"
            value={props.title}
            placeholder="Title"
            className={inputTextClassNames}
        />
        <input
            onInput={e => onUrl(e.currentTarget.value)}
            type="text"
            value={props.url}
            placeholder="Url"
            className={inputTextClassNames}
        />
        <input
            onInput={e => onDescription(e.currentTarget.value)}
            type="text"
            value={props.description}
            placeholder="Description"
            className={inputTextClassNames}
        />
        <div className="text-sm flex"><span className="my-auto">{count}/{total}</span></div>
        <div className="text-sm flex"></div>
        <div className="text-sm flex"><Button onClick={() => props.onNewBookmark()} className="my-auto font-bold bg-green-600 hover:bg-green-700  px-3 py-1">New</Button></div>
        <div className="ml-auto text-sm flex">
            <div className="flex align-sub my-auto">
                <div className="text-gray-100 flex mr-2">
                    <input
                        id="fetch-meta"
                        onChange={e => null}
                        type="checkbox"
                        autoComplete="off"
                        autoCorrect="off"
                        checked={true}
                        placeholder="Tags"
                        className={inputTextClassNames + " w-auto mr-2 text-left"}
                    />
                    <label htmlFor="fetch-meta" className="w-full">Ignore hidden</label>
                </div>

                <div><Button onClick={() => props.onColumns(Math.max(1, props.columns - 1))} >-</Button></div>
                <div className="mx-1">{props.columns}</div>
                <div><Button onClick={() => props.onColumns(Math.min(12, props.columns + 1))}>+</Button></div>
            </div>


        </div>
    </div>
}

export default Header;
