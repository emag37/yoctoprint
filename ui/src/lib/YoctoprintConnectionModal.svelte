<style>
h2 {
        font-size: 2rem;
        text-align: center;
    }

.fade-in {
    -webkit-animation: fade 5s infinite;
    -moz-animation: fade 5s infinite;
    -o-animation: fade 5s infinite;
    animation: fade 5s infinite;
}

@keyframes fade {
  0% {opacity:1;}
  25% {opacity:0.4;}
  50%  {opacity:1;}
  75%  {opacity:0.4;}
  100% {opacity:1;}
}

@-o-keyframes fade{
  0% {opacity:1;}
  25% {opacity:0.4;}
  50%  {opacity:1;}
  75%  {opacity:0.4;}
  100% {opacity:1;}
}
@-moz-keyframes fade{
  0% {opacity:1;}
  25% {opacity: 0.4;}
  50%  {opacity:1;}
  75%  {opacity:0.4;}
  100% {opacity:1;}
}
@-webkit-keyframes fade{
  0% {opacity:1;}
  25% {opacity:0.4;}
  50%  {opacity:1;}
  75%  {opacity:0.4;}
  100% {opacity:1;}
}

</style>

<h2 class="fade-in">Connecting to Yoctoprint...</h2>