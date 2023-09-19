import cookies from 'js-cookie'
import axios from 'axios'
import md5 from 'js-md5'
import { ElMessage } from 'element-plus'

let password = "";

function setPassword(pwd) {
    password = pwd;
    cookies.set('password', pwd, { expires: 365 })
}

function getPassword() {
    if (password === "") {
        password = cookies.get('password')
    }

    if (password === undefined) {
        password = "";
    }

    return password;
}

function clearPassword() {
    password = "";
    cookies.remove('password')
    window.location.reload();
}

function checkPassword(pwd) {
    axios.post('/check_password', {
        "password": md5(pwd)
    })
        .then(function (response) {
            if (response.data.code === 0) {
                setPassword(pwd);
                window.location.reload();
            } else {
                ElMessage.error(response.data.message);
            }
        })
        .catch(function (error) {
            console.log(error);
            ElMessage.error("Network error");
        }
        );
}

export {
    setPassword,
    getPassword,
    clearPassword,
    checkPassword
}