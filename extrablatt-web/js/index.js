import("../pkg/index.js").catch(console.error);

document.getElementById("btn-extract").addEventListener("click", extract, false);


 async function extract() {
    const val = document.getElementById("url-to-extract").value
    alert(val);
}