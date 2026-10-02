async function fetchJson(url) {
    const res = await fetch(url);
    const data = await res.json();
    return data;
}

const { projects } = await fetchJson("/projects");

let selectedProject = 0;
let selectedJobId = 0;
let jobIds = [];

let jobDetails = null;

async function setSelectedProject(index) {
    selectedProject = index;
    jobIds = [];
    renderSidebar();

    if (projects[selectedProject] === undefined) {
        return;
    }

    const { jobs } = await fetchJson(
        `/project/${projects[selectedProject].name}/jobs`,
    );

    jobIds = jobs;
    setSelectedJobId(0);
}

async function setSelectedJobId(index) {
    selectedJobId = index;
    renderSidebar();

    if (jobIds[selectedJobId] === undefined) {
        return;
    }

    jobDetails = await fetchJson(`/job/${jobIds[selectedJobId]}/details`);

    renderJobDetails();
}

function renderJobDetails() {
    const divJobDetails = document.getElementById("job_details");
    divJobDetails.replaceChildren();

    const projectHeader = document.createElement("h2");
    projectHeader.innerText = jobDetails.project;
    divJobDetails.appendChild(projectHeader);

    const tasksTable = document.createElement("table");
    for (const task of jobDetails.tasks) {
        const taskRow = document.createElement("tr");

        const taskTypeCol = document.createElement("td");
        taskTypeCol.innerText = task.type;
        taskRow.appendChild(taskTypeCol);

        const taskNameCol = document.createElement("td");
        taskNameCol.innerText = task.name;
        taskRow.appendChild(taskNameCol);

        const taskStatusCol = document.createElement("td");
        taskStatusCol.innerText = task.status;
        taskRow.appendChild(taskStatusCol);

        tasksTable.appendChild(taskRow);
    }
    divJobDetails.appendChild(tasksTable);

    console.log(jobDetails);
}

function renderSidebar() {
    const divSidebar = document.getElementById("sidebar_content");
    divSidebar.replaceChildren();

    for (const [index, project] of projects.entries()) {
        const projectButton = document.createElement("button");
        projectButton.innerText = project.name;
        projectButton.onclick = () => setSelectedProject(index);

        const projectDiv = document.createElement("div");
        projectDiv.appendChild(projectButton);

        if (index == selectedProject) {
            projectButton.classList.add("active");

            for (const [index, jobId] of jobIds
                .toSorted()
                .reverse()
                .entries()) {
                const jobButton = document.createElement("button");
                jobButton.innerText = jobId;
                jobButton.onclick = () => setSelectedJobId(index);

                if (index == selectedJobId) {
                    jobButton.classList.add("active");
                }

                projectDiv.appendChild(jobButton);
            }
        }

        divSidebar.appendChild(projectDiv);
    }
}

const es = new EventSource("/events");

setSelectedProject(0);
