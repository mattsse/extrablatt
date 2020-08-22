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


async function extract() {
    const url = document.getElementById("url-to-extract").value;
    // check url
    // fetch news

    clearTable();

    const table = generateTable({url: "dummy"});

    document.getElementById("extract-root").appendChild(table);
    return false;
}

function clearTable() {
    const table = document.getElementById("extract-table")
    if (table) {
        table.remove();
    }
}

function generateTable(data) {
    const tableRoot = document.createElement("div");
    tableRoot.setAttribute("id", "extract-tableRoot");
    // set title
    tableRoot.innerHTML = "<h3>" + "Extracted data" + "</h3>";
    tableRoot.innerHTML += "<p>" + data.url + "</p>"

    const table = document.createElement("table");
    tableRoot.appendChild(table);

    let thead = table.createTHead();

    return tableRoot
}


setUrlFromLocation();