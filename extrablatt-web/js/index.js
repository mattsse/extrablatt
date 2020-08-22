import {reqwest_article} from "../pkg";

import("../pkg/index.js").catch(console.error);

document.getElementById("btn-extract").addEventListener("click", extract, false);


// if a url query parameter is present we execute extract directly
function setUrlFromLocation() {
    const urlParams = new URLSearchParams(window.location.search);
    const url = urlParams.get("url");
    if (url) {
        document.getElementById("url-to-extract").value = decodeURI(url);
        document.getElementById("btn-extract").click();
    }
}

function setLocationFromUrl(url) {
    let param = "?url=" + encodeURI(url);
    window.history.pushState({}, url, param);
}

async function extract() {
    const url = document.getElementById("url-to-extract").value;
    clearTable();
    if (url) {
        setLocationFromUrl(url)
        try {
            const article = await reqwest_article(url);
            const table = generateTable(url, article);
            document.getElementById("extract-root").appendChild(table);
        } catch (e) {
            const msg = setError(e, url);
            document.getElementById("extract-root").appendChild(msg);
        }
    }
}

function clearTable() {
    const table = document.getElementById("extract-tableRoot");
    if (table) {
        table.remove();
    }
}

function setError(err, url) {
    if (err) {
        if (err.includes("Failed to fetch")) {
            err = err + ", Likely blocked by client due to CORS..."
        }
    }
    const msg = document.createElement("div");
    msg.setAttribute("id", "extract-tableRoot");
    msg.innerHTML = "<h3>" + err + "</h3>";
    msg.innerHTML += "<p>" + url + "</p>"
    return msg;
}

function generateTable(url, article) {
    const tableRoot = document.createElement("div");
    tableRoot.setAttribute("id", "extract-tableRoot");
    // set title
    const title = document.createElement("h3");
    title.innerText = "Extracted Data";
    tableRoot.appendChild(title);
    tableRoot.appendChild(createLinkCaption(url))

    const table = document.createElement("table");
    table.setAttribute("class", "table table-striped table-bordered");

    const tBody = table.createTBody();
    createTextValueRow(tBody.insertRow(), "Title", article.title);

    let authors = article.authors;
    if (authors) {
        authors = authors.join(", ")
    }
    createTextValueRow(tBody.insertRow(), "Authors", authors);

    let published = article.publishing_date;
    let updated = null;
    if (published) {
        updated = published.last_updated;
        published = published.published.DateTime;
        if (updated) {
            updated = updated.DateTime;
        }
    }
    createTextValueRow(tBody.insertRow(), "Published", published);
    createTextValueRow(tBody.insertRow(), "Updated", updated);

    let keywords = article.keywords;
    if (authors) {
        keywords = keywords.join(", ")
    }
    createTextValueRow(tBody.insertRow(), "Keywords", keywords);

    createTextValueRow(tBody.insertRow(), "Description", article.description);

    createImageRow(tBody.insertRow(), "Top Image", article.top_image)

    createArticleTextRow(tBody.insertRow(), article.text);

    createReferences(tBody.insertRow(), article.references);

    createImages(tBody.insertRow(), article.images);

    createVideos(tBody.insertRow(), article.videos);

    tableRoot.appendChild(table);
    return tableRoot
}

function createReferences(row, references) {
    let cell = row.insertCell();
    let text = document.createTextNode("References");
    cell.appendChild(text);
    cell = row.insertCell();
    if (references) {
        for (let i = 0; i < references.length; i++) {
            const caption = createLinkCaption(references[i]);
            cell.appendChild(caption);
        }
    }
}

function createVideos(row, videos) {
    let cell = row.insertCell();
    let text = document.createTextNode("Videos");
    cell.appendChild(text);
    cell = row.insertCell();
    if (videos) {
        for (let i = 0; i < videos.length; i++) {
            const caption = createLinkCaption(videos[i]);
            cell.appendChild(caption);
        }
    }
}

function createImages(row, images) {
    let cell = row.insertCell();
    let text = document.createTextNode("Images");
    cell.appendChild(text);
    cell = row.insertCell();
    if (images) {
        for (let i = 0; i < images.length; i++) {
            const val = createImageAndCaption(images[i]);
            cell.appendChild(val.img);
            cell.appendChild(val.caption);
        }
    }
}

function createArticleTextRow(row, txt) {
    let cell = row.insertCell();
    let text = document.createTextNode("Text");
    cell.appendChild(text);
    cell = row.insertCell();
    if (txt) {
        const lines = txt.split(/\n/);

        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            if (line) {
                const p = document.createElement("p");
                p.innerText = line;
                cell.appendChild(p);
            }
        }
    }
}

function createTextValueRow(row, name, value) {
    let cell = row.insertCell();
    let text = document.createTextNode(name);
    cell.appendChild(text);
    cell = row.insertCell();
    if (value) {
        text = document.createTextNode(value);
        cell.appendChild(text);
    }
}

function createImageAndCaption(url) {
    const img = document.createElement("img");
    img.setAttribute("src", url);
    const caption = createLinkCaption(url);
    caption.setAttribute("class", "text-center");
    return {img, caption}
}

function createImageRow(row, name, url) {
    let cell = row.insertCell();
    const text = document.createTextNode(name);
    cell.appendChild(text);
    cell = row.insertCell();
    if (url) {
        const val = createImageAndCaption(url);
        cell.appendChild(val.img);
        cell.appendChild(val.caption);
    }
}

function createLinkCaption(url) {
    const caption = document.createElement("p");
    const link = document.createElement("a");
    link.setAttribute("href", url);
    link.innerText = url;
    caption.appendChild(link);
    return caption;
}


setUrlFromLocation();