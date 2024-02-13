const { listen } = window.__TAURI__.event;

/* Drag and drop files into file explorer */
listen('tauri://file-drop', event => {
    event.payload.forEach(item => {
        console.log(item);
        CopyFilePath = item;
        let tempCopyFilePath = CopyFilePath.split("/");
        CopyFileName = tempCopyFilePath[tempCopyFilePath.length - 1].replace("'", "");
        let element = document.createElement("button");
        element.setAttribute("itemname", CopyFileName);
        element.setAttribute("itempath", CopyFilePath);
        ArrSelectedItems.push(element);
        ArrCopyItems.push(element);
    });
    pasteItem();
})


/* Toasts */
function showToast(title, message, type = "info") {
    let toast = document.createElement("div");
    toast.className = `toast toast-${type}`;
    let iconClass = "fa-solid fa-info-circle";

    if (type == "success") {
        iconClass = "fa-solid fa-circle-check";
    }
    else if (type == "error") {
        iconClass = "fa-solid fa-triangle-exclamation";
    }

    toast.innerHTML = `
        <div class="toast-icon toast-icon-${type}">
            <i class="fa-regular ${iconClass}"></i>
        </div>
        <div class="toast-content">
            <h5>
                <span>${title}</span>
            </h5>
            <p>${message}</p>
        </div>
    `;

    $(".toast-container").append(toast);
    setTimeout(() => {
        toast.style.opacity = 0;
        toast.style.translate = "100px";
    }, 2000);

    setTimeout(() => {
        toast?.remove();
    }, 2200);
}
